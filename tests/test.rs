mod common;

// Include tests directly in this file if desired
#[cfg(test)]
mod tests {
    // use rust_vmmap::*;

    use crate::common;

    #[test]
    fn tests() {
        common::setup();
        //TODO: implement integration test
    }
}
