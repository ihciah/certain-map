use certain_map::{certain_map, Param, ParamRef, ParamRemove, ParamSet, ParamTake};

#[derive(Clone)]
pub struct UserName(String);

#[derive(Copy, Clone)]
pub struct UserAge(u8);

certain_map! {
    #[empty(MyCertainMapEmpty)]
    #[full(MyCertainMapFull)]
    #[derive(Clone)]
    pub struct MyCertainMap {
        name: UserName,
        #[ensure(Clone)]
        age: UserAge,
    }
}

fn main() {
    let mut store = MyCertainMap::new();
    let meta = store.handler();

    // With #[default(MyCertainMapEmpty)] we can get an empty type.
    assert_type::<MyCertainMapEmpty>(&meta);

    // The following line compiles fail since there's no UserName in the map.
    // log_username(&meta);

    let meta = meta.param_set(UserName("ihciah".to_string()));
    // Now we can get it with certainty.
    log_username(&meta);

    // Fork the store and handler(like Clone).
    let (mut store_forked, state_forked) = meta.fork();
    let meta_forked = unsafe { state_forked.attach(&mut store_forked) };

    let (meta, removed) = ParamTake::<UserName>::param_take(meta);
    assert_eq!(removed.0, "ihciah");
    // The following line compiles fail since the UserName is removed.
    // log_username(&meta);
    // It does not affect forked meta.
    log_username(&meta_forked);

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

fn assert_type<T>(_: &T) {}
