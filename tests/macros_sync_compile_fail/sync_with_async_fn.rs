// This file should FAIL to compile because `#[cached(sync)]` cannot be
// combined with `async fn`. The macro panics at expansion time with:
//   "`#[cached(sync)]` cannot be used with `async fn` ..."

use oxcache::cached;

#[cached(service = "compile_fail_test", sync)]
async fn get_user() -> Result<(), ()> {
    Ok(())
}

fn main() {}
