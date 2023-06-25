# Certain Map
[![Crates.io](https://img.shields.io/crates/v/certain-map.svg)](https://crates.io/crates/certain-map)

A typed map which can make sure item exist.

## What Problem Does It Solve
In Rust, we often use Service abstraction for modular structure design(for example [tower-service](https://crates.io/crates/tower-service) or [service-async](https://github.com/ihciah/service-async)).

Services are stacked, and the Request/Response types of different layers may be different.

When components in different layers have data dependencies, especially indirect dependencies, it becomes difficult to pass all the required information by changing the Request/Response type.

When the number of variables to be passed increases or decreases, we must redefine a struct to load these quantities, and need to implement conversion functions and data extraction functions for these structs. This will be a huge and boring job, and it will also make the code become Dirtier.

So usually we don't do this, we often use HashMap or TypeMap to manage the information that needs to be passed across Services.

But this will bring an obvious problem: we cannot ensure at compile time that the key-value required by subsequent components has been set when it is read. This can lead to unreasonable error handling branches in our program or panic in some scenarios.

## What Benifits
In this crate, we transform struct type when keys inserted or removed. So we can makes sure some value must exist at compile-time.

When you need to use a structure to pass information between multiple stages, this crate will be most suitable for you.

This re-fulfills the promise: if it compiles, it works.

## Usage
```rust
use certain_map::{certain_map, Param, ParamRef, ParamRemove, ParamSet, ParamTake};

struct UserName(String);

#[derive(Copy, Clone)]
struct UserAge(u8);

certain_map! {
    pub struct MyCertainMap {
        name: UserName,
        #[ensure(Clone)]
        age: UserAge,
    }
}

fn main() {
    let meta = MyCertainMap::new();

    // The following line compiles fail since there's no UserName in the map.
    // log_username(&meta);

    let meta = meta.param_set(UserName("ihciah".to_string()));
    // Now we can get it with certainty.
    log_username(&meta);

    let (meta, removed) = ParamTake::<UserName>::param_take(meta);
    assert_eq!(removed.0, "ihciah");
    // The following line compiles fail since the UserName is removed.
    // log_username(&meta);

    // We can also remove a type no matter if it exist.
    let meta = ParamRemove::<UserName>::param_remove(meta);

    let meta = meta.param_set(UserAge(24));
    // we can get ownership of fields with #[ensure(Clone)]
    log_age(&meta);
}

fn log_username<T: ParamRef<UserName>>(meta: &T) {
    println!("username: {}", meta.param_ref().0);
}

fn log_age<T: Param<UserAge>>(meta: &T) {
    println!("user age: {}", meta.param().0);
}

```