// Copyright 2024 ihciah. All Rights Reserved.

//! This demo is used to show how to use certain_map with `Services`(no matter tower Service or
//! service-async Service).
//! To pass information across service layers, and make it able to decouple the Context concrete
//! type, we can use certain_map with param crate.

use std::{convert::Infallible, future::Future, marker::PhantomData};

use certain_map::Handler;
use certain_map_macros::certain_map;
use param::{ParamRef, ParamSet};

// Copied from https://github.com/ihciah/service-async/blob/master/service-async/src/lib.rs
// Copy to avoid adding dev-dependency.
trait Service<Request> {
    type Response;
    type Error;
    fn call(&self, req: Request) -> impl Future<Output = Result<Self::Response, Self::Error>>;
}

#[derive(Clone)]
pub struct RawBeforeAdd(u8);

#[derive(Clone)]
pub struct RawBeforeMul(u8);

certain_map! {
    #[empty(MyCertainMapEmpty)]
    #[full(MyCertainMapFull)]
    #[derive(Clone)]
    pub struct MyCertainMap {
        raw_before_add: RawBeforeAdd,
        raw_before_mul: RawBeforeMul,
    }
}

// Define a service that adds 1 to the input number.
struct Add1<T>(T);

impl<T, CX> Service<(u8, CX)> for Add1<T>
where
    T: Service<(u8, CX::Transformed)>,
    CX: ParamSet<RawBeforeAdd>,
{
    type Response = T::Response;
    type Error = T::Error;

    fn call(
        &self,
        (num, cx): (u8, CX),
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> {
        self.0.call((num + 1, cx.param_set(RawBeforeAdd(num))))
    }
}

// Define a service that multiplies the input number by 2.
struct Mul2<T>(T);

impl<T, CX> Service<(u8, CX)> for Mul2<T>
where
    T: Service<(u8, CX::Transformed)>,
    CX: ParamSet<RawBeforeMul>,
{
    type Response = T::Response;
    type Error = T::Error;

    fn call(
        &self,
        (num, cx): (u8, CX),
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> {
        self.0.call((num * 2, cx.param_set(RawBeforeMul(num))))
    }
}

// Define a service that prints the context and return the input.
struct Identical;

impl<CX> Service<(u8, CX)> for Identical
where
    CX: ParamRef<RawBeforeAdd> + ParamRef<RawBeforeMul>,
{
    type Response = u8;
    type Error = Infallible;

    async fn call(&self, (num, cx): (u8, CX)) -> Result<Self::Response, Self::Error> {
        println!(
            "num before add: {}",
            ParamRef::<RawBeforeAdd>::param_ref(&cx).0
        );
        println!(
            "num before mul: {}",
            ParamRef::<RawBeforeMul>::param_ref(&cx).0
        );
        println!("num: {num}");
        Ok(num)
    }
}

// A service that create a context and call the inner service.
struct CXSvc<CXStore, T> {
    inner: T,
    cx: PhantomData<CXStore>,
}

impl<CXStore, T> CXSvc<CXStore, T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            cx: PhantomData,
        }
    }
}

impl<CXStore, T> CXSvc<CXStore, T> {
    async fn call<R, RESP, ERR>(&self, num: R) -> Result<RESP, ERR>
    where
        CXStore: Handler + Default + 'static,
        for<'a> T: Service<(R, CXStore::Hdr<'a>), Response = RESP, Error = ERR>,
    {
        let mut store = CXStore::default();
        let hdr = store.handler();
        self.inner.call((num, hdr)).await
    }
}

#[tokio::main]
async fn main() {
    // Create service and initialize store, then call with the handler.
    // (2 + 1) * 2 = 6 is expected.
    let svc = Add1(Mul2(Identical));
    let mut store = MyCertainMap::new();
    assert_eq!(svc.call((2, store.handler())).await.unwrap(), 6);

    // You can even create a service to initialize store and pass the handler.
    let svc = CXSvc::<MyCertainMap, _>::new(svc);
    assert_eq!(svc.call(2).await.unwrap(), 6);
}
