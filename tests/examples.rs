use backtracked_micropass_type_inference::{
    Checker, Environment, Error, Finalizer, InferenceUnifier, KnownType as T, KnownType, Map,
    VariableKey,
};
use simple_logger::SimpleLogger;
use std::sync::Once;

static LOGGER: Once = Once::new();

fn process(mut env: Environment, expected_errors: Vec<Error>) -> Map<VariableKey, KnownType> {
    InferenceUnifier::new(&mut env).infer();

    let assignments = Finalizer::new(&mut env).finalize_all();

    let errors = Checker::new(&assignments, &mut env).type_check();

    let mut fail = false;

    for err in &errors {
        eprintln!("{err}");
        fail |= !expected_errors.contains(&err);
    }

    for err in expected_errors {
        if !errors.contains(&err) {
            eprintln!("missing error:\n{err}");
            fail = true;
        }
    }

    if fail {
        panic!("one or more error checks failed");
    }

    assignments
}

fn logger() {
    LOGGER.call_once(|| {
        SimpleLogger::new()
            .with_level(log::LevelFilter::Info)
            .init()
            .unwrap();
    });
}

#[test]
fn static_function_application() {
    logger();
    let mut env = Environment::new();
    env.leave_signature_enter_expression();

    // let f = |x: i32, y: i32| -> i32
    let func = {
        let params = vec![env.i(32), env.i(32)];
        let ret = env.i(32);
        env.function(params, ret)
    };

    // let r = f(1_i32, 2_i32)
    let ret = {
        let application = env.apply(func);
        let i32 = env.i(32);
        env.apply_next_parameter(application, i32);
        env.apply_next_parameter(application, i32);
        env.get_return_type(application)
    };

    // return r
    let expected_return_type = env.i(64);
    env.assign(ret, expected_return_type);

    process(
        env,
        vec![Error::Mismatch {
            expected: T::i(64),
            given: T::i(32),
            message: "can not be assigned to this type".into(),
        }],
    );
}

#[test]
fn generic_function_application() {
    logger();
    let mut env = Environment::new();
    env.leave_signature_enter_expression();

    // let map = |list: [a], f: (a -> b)| -> [b]
    let func = {
        let a = || T::generic("a");
        let b = || T::generic("b");
        let f = T::function([a()], b());
        env.instantiate_function(["a", "b"], &[T::list(a()), f], &T::list(b()))
    };

    // let r = map([5_i32], |n: i32| n as i64);
    let ret = {
        let _i32 = env.i(32);
        let _i64 = env.i(64);
        let list = env.list(_i32);
        let f = env.function(vec![_i32], _i64);

        let application = env.apply(func);
        env.apply_next_parameter(application, list);
        env.apply_next_parameter(application, f);

        env.get_return_type(application)
    };

    // return r
    let expected_return_type = {
        let _i8 = env.i(8);
        env.list(_i8)
    };
    env.assign(ret, expected_return_type);

    process(
        env,
        vec![Error::Mismatch {
            expected: T::list(T::i(8)),
            given: T::list(T::i(64)),
            message: "can not be assigned to this type".into(),
        }],
    );
}

#[test]
fn list() {
    logger();
    let mut env = Environment::new();
    env.leave_signature_enter_expression();

    // [1_i32, 2_i32, "3"]
    let _list = {
        let (sameas_key, list, _) = env.list_sameas();

        let _i32 = env.i(32);
        let string = env.string();

        env.add_sameas_member(sameas_key, _i32);
        env.add_sameas_member(sameas_key, _i32);
        env.add_sameas_member(sameas_key, string);

        list
    };

    process(
        env,
        vec![Error::Mismatch {
            expected: T::i(32),
            given: T::string(),
            message: "type must be same as the other types of this list".into(),
        }],
    );
}

#[test]
fn if_expr() {
    logger();
    let mut env = Environment::new();
    env.leave_signature_enter_expression();

    // if true { 20_i32 } else { "20" }
    let _expr = {
        let (sameas_key, expr) = env.expr_sameas();

        let _i32 = env.i(32);
        let string = env.string();

        env.add_sameas_member(sameas_key, _i32);
        env.add_sameas_member(sameas_key, string);

        expr
    };

    process(
        env,
        vec![Error::Mismatch {
            expected: T::i(32),
            given: T::string(),
            message: "type must be same as the other branches of this expression".into(),
        }],
    );
}

#[test]
fn field_inference() {
    logger();
    let mut env = Environment::new();
    env.leave_signature_enter_expression();

    // let point;
    // point.x = 4_i32;
    let (_record, x) = {
        let point = env.unknown();
        let x = env.add_field(point, "x");
        let _i32 = env.i(32);
        env.assign(_i32, x);
        (point, x)
    };

    // return point.x
    let expected_return_type = env.i(64);
    env.assign(x, expected_return_type);

    process(
        env,
        vec![Error::Mismatch {
            expected: T::i(64),
            given: T::i(32),
            message: "can not be assigned to this type".into(),
        }],
    );
}

#[test]
fn defaulting() {
    logger();
    let mut env = Environment::new();

    // x: _, y: _ // in function signature
    let x = env.unknown();
    let y = env.unknown();

    env.leave_signature_enter_expression();

    // let n = 4 // this number isn't used. So; we don't know which numeric type it is
    let n = env.numeric();

    // let z
    let z = env.unknown();

    let assignments = process(env, vec![]);
    assert_eq!(assignments[x], T::generic("a"));
    assert_eq!(assignments[y], T::generic("b"));
    assert_eq!(assignments[n], T::default_int());
    assert_eq!(assignments[z], T::default_unit_type());
}
