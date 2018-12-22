// TODO: Yes,

pub type GcType<T> = *mut T;

pub fn new<T>(val: T) -> GcType<T> {
    Box::into_raw(Box::new(val))
}
