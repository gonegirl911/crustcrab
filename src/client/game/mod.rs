pub mod scene;

use self::scene::Scene;
use super::{
    event_loop::{Event, EventHandler},
    renderer::Renderer,
    ClientEvent,
};
use flume::Sender;
use std::time::Duration;
use winit::event_loop::ControlFlow;

pub struct Game {
    scene: Scene,
}

impl Game {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            scene: Scene::new(renderer),
        }
    }
}

impl EventHandler for Game {
    type Context<'a> = (
        &'a mut ControlFlow,
        Sender<ClientEvent>,
        &'a Renderer,
        Duration,
    );

    fn handle(
        &mut self,
        event: &Event,
        (
            control_flow,
            client_tx,
            renderer @ Renderer {
                surface,
                device,
                queue,
                ..
            },
            dt,
        ): Self::Context<'_>,
    ) {
        self.scene.handle(event, (client_tx, renderer, dt));

        if let Event::RedrawRequested(_) = event {
            match surface.get_current_texture() {
                Ok(output) => {
                    let mut encoder = device.create_command_encoder(&Default::default());
                    self.scene.render(
                        &output.texture.create_view(&Default::default()),
                        &mut encoder,
                    );
                    queue.submit([encoder.finish()]);
                    output.present();
                }
                Err(wgpu::SurfaceError::Lost) => renderer.refresh(),
                Err(wgpu::SurfaceError::OutOfMemory) => control_flow.set_exit(),
                _ => {}
            };
        }
    }
}
