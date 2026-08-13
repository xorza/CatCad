//! CatCad application entry point.

use catcad::CatCad;
use palantir::{WindowToken, WinitHost, WinitHostError};

fn main() -> Result<(), WinitHostError> {
    WinitHost::builder(WindowToken(0))
        .title("CatCad")
        .build(CatCad::new)?
        .run()
}
