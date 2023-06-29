use certain_map::{certain_map, Param, ParamRef, ParamRemove, ParamSet, ParamTake};

pub struct UserName(String);

#[derive(Copy, Clone)]
pub struct UserAge(u8);

certain_map! {
    #[empty(MyCertainMapEmpty)]
    #[full(MyCertainMapFull)]
    pub struct MyCertainMap {
        name: UserName,
        #[ensure(Clone)]
        age: UserAge,
    }
}

fn main() {
    let meta = MyCertainMap::new();

    // With #[default(MyCertainMapEmpty)] we can get an empty type.
    assert_type::<MyCertainMapEmpty>(&meta);

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

fn assert_type<T>(_: &T) {}
