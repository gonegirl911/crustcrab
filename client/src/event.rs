use domain::protocol::{chunk, control};
use winit::event::{DeviceEvent, WindowEvent};

pub(super) enum Event {
    Init,
    Control(control::Event),
    Chunk(chunk::Event),
    WindowEvent(WindowEvent),
    DeviceEvent(DeviceEvent),
    AboutToWait,
    MemoryWarning,
    InputInvalidated,
}

pub(super) trait Handler {
    type Context<'a>;

    fn handle(&mut self, event: &Event, cx: Self::Context<'_>);
}
