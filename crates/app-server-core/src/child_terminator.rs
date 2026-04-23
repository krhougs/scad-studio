use std::process::Child;

pub trait ChildTerminator {
    fn terminate(&self, child: &mut Child) -> std::io::Result<()>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultChildTerminator;

impl ChildTerminator for DefaultChildTerminator {
    fn terminate(&self, child: &mut Child) -> std::io::Result<()> {
        child.kill()
    }
}

pub fn terminate_child(child: &mut Child) {
    let terminator = DefaultChildTerminator;
    let _ = terminate_child_with(child, &terminator);
}

pub fn terminate_child_with(
    child: &mut Child,
    terminator: &impl ChildTerminator,
) -> std::io::Result<()> {
    terminator.terminate(child)
}
