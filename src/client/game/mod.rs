pub mod gui;
pub mod player;
pub mod scene;

use self::{gui::Gui, player::Player, scene::Scene};
use super::{
    event_loop::{Event, EventHandler},
    renderer::{PostProcessor, Renderer},
    ClientEvent,
};
use flume::Sender;
use std::time::Duration;

pub struct Game {
    player: Player,
    scene: Scene,
    gui: Gui,
    processor: PostProcessor,
}

impl Game {
    pub fn new(renderer: &Renderer) -> Self {
        let processor = PostProcessor::new(renderer);
        let gui = Gui::new(renderer, processor.bind_group_layout());
        let player = Player::new(renderer, &gui);
        let scene = Scene::new(renderer, player.bind_group_layout());
        Self {
            player,
            scene,
            gui,
            processor,
        }
    }

    fn draw(&mut self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        self.scene.draw(
            self.processor.view(),
            encoder,
            self.player.bind_group(),
            &self.player.frustum(),
        );
        self.processor.apply(encoder, &self.gui);
        self.processor.draw(view, encoder);
    }
}

impl EventHandler for Game {
    type Context<'a> = (Sender<ClientEvent>, &'a Renderer, Duration);

    #[rustfmt::skip]
    fn handle(
        &mut self,
        event: &Event,
        (
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
        self.player.handle(event, (client_tx, renderer, &self.gui, dt));
        self.scene.handle(event, renderer);
        self.gui.handle(event, renderer);
        self.processor.handle(event, renderer);

        if let Event::RedrawRequested(_) = event {
            match surface.get_current_texture() {
                Ok(surface) => {
                    let view = surface.texture.create_view(&Default::default());
                    let mut encoder = device.create_command_encoder(&Default::default());
                    self.draw(&view, &mut encoder);
                    queue.submit([encoder.finish()]);
                    surface.present();
                }
                Err(wgpu::SurfaceError::Lost) => renderer.recreate_surface(),
                Err(_) => {},
            }
        }
    }
}
