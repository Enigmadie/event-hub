#[derive(Debug, Clone)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new(value: String) -> Self {
        Self(value)
    }
}
