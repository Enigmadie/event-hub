#[derive(Debug, Clone)]
pub struct DeviceName(String);

impl DeviceName {
    pub fn new(value: String) -> Self {
        Self(value)
    }
}
