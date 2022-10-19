use wgpu::{
    Backends, BlendState, Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
    CompositeAlphaMode, Device, DeviceDescriptor, Face, Features, FragmentState, FrontFace,
    Instance, Limits, LoadOp, MultisampleState, Operations, PipelineLayoutDescriptor, PolygonMode,
    PowerPreference, PresentMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource, Surface, SurfaceConfiguration,
    SurfaceError, TextureUsages, TextureViewDescriptor, VertexState,
};
use winit::{dpi::PhysicalSize, event::WindowEvent, window::Window};

pub struct State {
    pub surface: Surface,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
    pub render_pipeline: RenderPipeline,
}

impl State {
    /// Too much stuff in here
    pub async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        // The `instance` is the handle to our GPU, used to create `Adapter`s and `Surface`s
        let instance = Instance::new(Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        // The `adapter` is a handle to our actual graphics card, and can be used to fetch info about it, and we can use this to create the `Device` and `Queue`
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        dbg!(adapter.get_info());
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    features: Features::empty(),
                    limits: Limits::downlevel_defaults(),
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        let config = SurfaceConfiguration {
            // How `SurfaceTexture`s will be used, `RENDER_ATTACHMENT` means the textures will be used to write to the screen
            usage: TextureUsages::RENDER_ATTACHMENT,
            // How `SurfaceTexture`s will be stored on the GPU, different displays prefer different formats, so we use `surface.get_preferred_format()` to figure out the best format based on the display being used
            format: surface.get_supported_formats(&adapter)[0],
            // The size in pixels of a `SurfaceTexture` (should usually be the size of the window)
            width: size.width,
            height: size.height,
            // How to sync the surface with the display, `PresentMode::Fifo` will cap the display rate at the display's framerate, essentially VSync, which is guaranteed to be supported on all platforms
            present_mode: PresentMode::Fifo,
            // How the alpha channel of the textures should be handled during compositing (combining textures), `CompositeAlphaMode::Auto` picks depending on what the surface can support
            alpha_mode: CompositeAlphaMode::Auto,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shader"),
            source: ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &shader,
                // The function we marked with `@vertex`
                entry_point: "vs_main",
                // Tells `wgpu` what type of vertices we want to pass to the vertex shader
                // We specify the vertices in the vertex shader itself so we'll leave it empty
                buffers: &[],
            },
            // Technically optional
            fragment: Some(FragmentState {
                module: &shader,
                // The function we marked with `@fragment`
                entry_point: "fs_main",
                // Tells `wgpu` what colour outputs it should set up
                // We only need one for the `surface`
                targets: &[Some(ColorTargetState {
                    // We copy `surface`'s format so that copying to it is easy
                    format: config.format,
                    // Replace old pixel data with new data
                    blend: Some(BlendState::REPLACE),
                    // Write to all colours
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                // Every 3 vertices will correspond to 1 trongle
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                // How to determine whether a triangle is facing forwards (if its counter-clockwise)
                front_face: FrontFace::Ccw,
                // Cull any triangles facing backwards
                cull_mode: Some(Face::Back),
                // Setting this to anything other than `PolygonMode::Fill` requires `Features::NON_FILL_POLYGON_MODE`
                polygon_mode: PolygonMode::Fill,
                // Requires `Features::DEPTH_CLIP_CONTROL`
                unclipped_depth: false,
                // Requires `Features::CONSERVATIVE_RASTERIZATION`
                conservative: false,
            },
            // We're not using a depth/stencil buffer currently
            depth_stencil: None,
            multisample: MultisampleState {
                // How many samples the pipeline will use
                count: 1,
                // Which samples should be active
                mask: !0,
                // To do with anti-aliasing
                alpha_to_coverage_enabled: false,
            },
            // How many array layers the render attachments can have, we won't be rendering to array textures
            multiview: None,
        });

        // et voilà
        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
        }
    }

    /// Resize the surface with `new_size`
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            // Have to reconfigure the surface with the new width and height
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Indicates whether an event has been fully processed
    pub fn input(&mut self, _event: &WindowEvent) -> bool {
        false
    }

    pub fn update(&mut self) {}

    /// Where the magic happens
    pub fn render(&mut self) -> Result<(), SurfaceError> {
        let output =
            // Will wait for `self.surface` to provide a new `SurfaceTexture` to be rendered to
            self.surface.get_current_texture()?;
        // Creates a `TextureView` with the default settings
        // We need to do this because we want to control how the render code interacts with the texture
        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());
        // Most modern graphics libs expect commands to be stored in a command buffer before being sent to the GPU
        // The `encoder` builds a command buffer that we can then send to the GPU
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        // `begin_render_pass()` performs a mutable borrow of `encoder`
        // We can't call `encoder.finish()` until we release the borrow
        // This is the purpose of the block: to drop the mutable borrow of `encoder`
        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Render Pass"),
                // Where we are going to draw our colour to, we use `view` to ensure we render to the screen
                color_attachments: &[Some(RenderPassColorAttachment {
                    // Which texture to save the colours to
                    view: &view,
                    // The texture that will recieve the resolved output, which will be the same as `view` unless mutli-sampling is enabled
                    // Since we don't need to specify this (because we're not using mutli-sampling), we leave it as `None`
                    resolve_target: None,
                    // Tells wgpu what to do with the colours on the screen
                    ops: Operations {
                        // How to handle colors stored from the previous frame, currently we are clearing the screen with a blueish colour
                        load: LoadOp::Clear(Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        // Whether we want to store the rendered results to the `Texture` behind `view`
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            // Draw something with 3 vertices, and 1 instance
            render_pass.draw(0..3, 0..1);
        }

        // submit will accept any `IntoIter`
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}
