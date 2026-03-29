use rhai::{Array, Dynamic};

pub trait ToArray {
    fn to_array(self) -> Array;
}

impl<T: Clone + 'static> ToArray for Vec<T> {
    fn to_array(self) -> Array {
        self.into_iter().map(Dynamic::from).collect::<Array>()
    }
}
