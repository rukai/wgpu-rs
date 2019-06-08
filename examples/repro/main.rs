use std::mem;
use std::thread;
use std::sync::mpsc;

#[derive(Clone, Copy)]
struct Vertex {
    _pos:   [f32; 4],
    _color: [f32; 4],
}

fn main() {
    env_logger::init();

    let width: u16 = 400;
    let height: u16 = 300;

    let mut state = create_state();

    for i in 0..100000 {
        let subaction_len = 70;
        println!("i: {}", i);
        let (frames_tx, frames_rx) = mpsc::channel();
        let (gif_tx, gif_rx) = mpsc::channel();

        thread::spawn(move || {
            let result: Vec<u8> = vec!();
            {
                for _k in 0..subaction_len {
                    //println!("k: {}", k);
                    let _ = frames_rx.recv().unwrap();
                }
            }
            gif_tx.send(result).unwrap();
        });

        for _j in 0..subaction_len {
            //println!("j: {}", j);
            let texture_extent = wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth: 1
            };
            let framebuffer_descriptor = &wgpu::TextureDescriptor {
                size: texture_extent,
                array_layer_count: 1,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            };

            let framebuffer = state.device.create_texture(framebuffer_descriptor);
            let framebuffer_copy_view = wgpu::TextureCopyView {
                texture: &framebuffer,
                mip_level: 0,
                array_layer: 0,
                origin: wgpu::Origin3d { x: 0.0, y: 0.0, z: 0.0 },
            };

            let framebuffer_out_usage = &wgpu::BufferDescriptor {
                size: width as u64 * height as u64 * 4,
                usage: wgpu::BufferUsage::MAP_READ | wgpu::BufferUsage::TRANSFER_DST,
            };
            let framebuffer_out = state.device.create_buffer(framebuffer_out_usage);
            let framebuffer_out_copy_view = wgpu::BufferCopyView {
                buffer: &framebuffer_out,
                offset: 0,
                row_pitch: 0,
                image_height: height as u32,
            };

            let mut command_encoder = draw_frame(&mut state, &framebuffer.create_default_view());
            command_encoder.copy_texture_to_buffer(framebuffer_copy_view, framebuffer_out_copy_view, texture_extent); // removing this line, makes the leak drastically better

            state.device.get_queue().submit(&[command_encoder.finish()]);

            let frames_tx = frames_tx.clone();
            //println!("map_read_async");
            framebuffer_out.map_read_async(0, width as u64 * height as u64 * 4, move |result: wgpu::BufferMapAsyncResult<&[u8]>| {
                match result {
                    Ok(data) => {
                        let data = data.data;
                        let pixel_count = width as usize * height as usize;
                        let mut result = data.to_vec();
                        for i in 0..pixel_count {
                              let b = result[i * 4 + 0];
                            //let g = result[i * 4 + 1];
                              let r = result[i * 4 + 2];
                            //let a = result[i * 4 + 3];

                              result[i * 4 + 0] = r;
                            //result[i * 4 + 1] = g;
                              result[i * 4 + 2] = b;
                            //result[i * 4 + 3] = a;
                        }
                        frames_tx.send(result).unwrap();
                    }
                    Err(error) => {
                        panic!("map_read_async failed: {:?}", error); // We have to panic here to avoid an infinite loop :/
                    }
                }
            });
        }

        loop {
            match gif_rx.try_recv() {
                Err(_) => {
                    // Needed to get the last map_read_async to run.
                    state.device.poll(true);
                }
                Ok(_) => {
                    break;
                }
            }
        }
    }
}

struct WgpuState {
    pub instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub vs_module: wgpu::ShaderModule,
    pub fs_module: wgpu::ShaderModule,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub render_pipeline: wgpu::RenderPipeline,
}

fn create_state() -> WgpuState {
    let instance = wgpu::Instance::new();
    let adapter = instance.get_adapter(&wgpu::AdapterDescriptor {
        power_preference: wgpu::PowerPreference::LowPower,
    });
    let device = adapter.request_device(&wgpu::DeviceDescriptor {
        limits: wgpu::Limits::default(),
        extensions: wgpu::Extensions {
            anisotropic_filtering: false,
        },
    });

    // shaders
    let vs_bytes = include_bytes!("fighter.vert.spv");
    let vs_module = device.create_shader_module(vs_bytes);
    let fs_bytes = include_bytes!("fighter.frag.spv");
    let fs_module = device.create_shader_module(fs_bytes);

    // layout
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[
            wgpu::BindGroupLayoutBinding {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_group_layout],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        layout: &pipeline_layout,
        vertex_stage: wgpu::PipelineStageDescriptor {
            module: &vs_module,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::PipelineStageDescriptor {
            module: &fs_module,
            entry_point: "main",
        }),
        rasterization_state: wgpu::RasterizationStateDescriptor {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        },
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        color_states: &[wgpu::ColorStateDescriptor {
            format: wgpu::TextureFormat::Bgra8Unorm,
            color_blend: wgpu::BlendDescriptor {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha_blend: wgpu::BlendDescriptor {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            write_mask: wgpu::ColorWrite::ALL,
        }],
        depth_stencil_state: None,
        index_format: wgpu::IndexFormat::Uint16,
        vertex_buffers: &[wgpu::VertexBufferDescriptor {
            stride: mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float4,
                    offset: 0,
                },
                wgpu::VertexAttributeDescriptor {
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float4,
                    offset: 4 * 4,
                },
            ],
        }],
        sample_count: 1,
    });

    WgpuState {
        instance,
        device,
        vs_module,
        fs_module,
        bind_group_layout,
        render_pipeline,
    }
}

fn draw_frame(state: &mut WgpuState, framebuffer: &wgpu::TextureView) -> wgpu::CommandEncoder {
    let mut command_encoder = state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

    {
        let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: &framebuffer,
                resolve_target: None,
                load_op: wgpu::LoadOp::Clear,
                store_op: wgpu::StoreOp::Store,
                clear_color: wgpu::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
            }],
            depth_stencil_attachment: None,
        });
        rpass.set_pipeline(&state.render_pipeline);

        for _l in 0..11 {
            //println!("l: {}", l);
            let mut indices_vec: Vec<u16> = vec!();
            for _ in 0..782 {
                indices_vec.extend(&[0, 1, 2]);
            }

            let _pos = [
                1.0,
                1.0,
                1.0,
                1.0
            ];
            let _color = [0.0, 0.0, 1.0, 0.3];

            let mut vertices_vec = vec!();
            for _ in 0..432 {
                vertices_vec.push(Vertex { _pos, _color });
            }

            let vertices = state.device.create_buffer_mapped(vertices_vec.len(), wgpu::BufferUsage::VERTEX)
                .fill_from_slice(&vertices_vec);
            let indices = state.device.create_buffer_mapped(indices_vec.len(), wgpu::BufferUsage::INDEX)
                .fill_from_slice(&indices_vec);

            let transform = [0.0f32; 16];
            let transform: &[f32; 16] = &transform;
            let uniform_buf = state.device
                .create_buffer_mapped(
                    16,
                    wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::TRANSFER_DST,
                )
                .fill_from_slice(transform);

            let bind_group = state.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &state.bind_group_layout,
                bindings: &[
                    wgpu::Binding {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer {
                            buffer: &uniform_buf,
                            range: 0..64,
                        },
                    },
                ],
            });

            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.set_index_buffer(&indices, 0);
            rpass.set_vertex_buffers(&[(&vertices, 0)]);
            rpass.draw_indexed(0..indices_vec.len() as u32, 0, 0..1);
        }
    }

    command_encoder
}
