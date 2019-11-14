use super::class::ClassRef;
use crate::vm::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct InstanceInfo {
    pub classref: ClassRef,
    pub class_name: String,
    pub instance_var: ValueTable,
}

impl InstanceInfo {
    pub fn new(classref: ClassRef, class_name: String) -> Self {
        InstanceInfo {
            classref,
            class_name,
            instance_var: HashMap::new(),
        }
    }

    pub fn get_classref(&self) -> ClassRef {
        self.classref
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceRef(*mut InstanceInfo);

impl std::hash::Hash for InstanceRef {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl InstanceRef {
    pub fn new(info: InstanceInfo) -> Self {
        let boxed = Box::into_raw(Box::new(info));
        InstanceRef(boxed)
    }
}

impl std::ops::Deref for InstanceRef {
    type Target = InstanceInfo;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl std::ops::DerefMut for InstanceRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}
