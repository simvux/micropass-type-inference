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
        #[cfg(debug_assertions)]
        eprintln!("{err}");
        fail |= !expected_errors.contains(&err);
    }

    for err in expected_errors {
        if !errors.contains(&err) {
            #[cfg(debug_assertions)]
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
        #[cfg(debug_assertions)]
        let level = log::LevelFilter::Info;
        #[cfg(not(debug_assertions))]
        let level = log::LevelFilter::Error;

        SimpleLogger::new().with_level(level).init().unwrap();
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

#[test]
fn article_showcase_example() {
    println!("{}", article_showcase_example_go());
}

#[test]
#[ignore]
fn inspect_article_showcase_example() {
    panic!("{}", article_showcase_example_go());
}

fn article_showcase_example_go() -> Map<VariableKey, KnownType> {
    logger();
    let mut env = Environment::new();
    let param0 = env.unknown();
    let param1 = env.i(32);
    let return_ = {
        let forall = [("a", env.string())].into();
        let fields = env.instantiate_fields("Just", &forall);
        env.record("Just", forall, fields)
    };
    env.leave_signature_enter_expression();

    // let z = {
    let z = {
        // let n = 4;
        let n = env.numeric();

        // let fst(param0, { x = n, y = n });

        let fst = env.instantiate_function(
            ["T", "U"],
            &[T::Generic("T"), T::Generic("U")],
            &T::Generic("T"),
        );

        let record = {
            let var = env.unknown();
            let x = env.add_field(var, "x");
            let y = env.add_field(var, "y");

            env.assign(n, x);
            env.assign(n, y);

            var
        };

        let appl = env.apply(fst);
        env.apply_next_parameter(appl, param0);
        env.apply_next_parameter(appl, record);

        env.get_return_type(appl)
    };

    // let _ = [z, param1];
    let _ = {
        let (sameas_key, list, _) = env.list_sameas();
        env.add_sameas_member(sameas_key, z);
        env.add_sameas_member(sameas_key, param1);
        list
    };

    // Maybe::Just(param0);
    let just = {
        let constructor = env.instantiate_function(
            ["a"],
            &[T::Generic("a")],
            &T::Record("Just", [("a", T::Generic("a"))].into()),
        );

        let appl = env.apply(constructor);
        env.apply_next_parameter(appl, param0);

        env.get_return_type(appl)
    };

    // return Maybe::Just(param0);
    env.assign(just, return_);

    process(
        env,
        vec![Error::Mismatch {
            expected: T::string(),
            given: T::default_int(),
            message: "type must be same as the other types of this list".into(),
        }],
    )
}

#[test]
fn article_feature_complete_example() {
    println!("{}", article_feature_complete_example_go());
}

#[test]
#[ignore]
fn inspect_article_feature_complete_example() {
    panic!("{}", article_feature_complete_example_go());
}

fn article_feature_complete_example_go() -> Map<VariableKey, KnownType> {
    logger();
    let mut env = Environment::new();

    let a = env.unknown();
    let _b = env.unknown();
    let return_ = env.unknown();

    env.leave_signature_enter_expression();

    // let point = Point(a, 200);
    let [_let_point, x] = {
        let (forall, fields) = env.instantiate_record("Point").unwrap();
        let func_params = vec![forall["a"], forall["a"]];
        let func_ret = env.record("Point", forall, fields);
        let func = env.function(func_params.clone(), func_ret);
        let _200 = env.numeric();

        let appl = env.apply(func);
        env.apply_next_parameter(appl, a);
        env.apply_next_parameter(appl, _200);

        let point = env.get_return_type(appl);

        let let_point = env.unknown();
        env.assign(point, let_point);
        let x = env.add_field(let_point, "x");

        [let_point, x]
    };

    // let list = [point.x, 300];
    {
        let list_literal = {
            let (same_as, list, _elem) = env.list_sameas();
            let _300 = env.numeric();
            env.add_sameas_member(same_as, x);
            env.add_sameas_member(same_as, _300);
            list
        };

        let list = env.unknown();
        env.assign(list_literal, list);
        list
    };

    // let record = { x = 100, y = 200 };
    let [_let_record, y] = {
        let record = env.unknown();
        let _100 = env.numeric();
        let _200 = env.numeric();
        let x = env.add_field(record, "x");
        let y = env.add_field(record, "y");
        env.assign(_100, x);
        env.assign(_200, y);

        let let_record = env.unknown();
        env.assign(record, let_record);
        [let_record, y]
    };

    // point.y == record.y
    let comparison = {
        let bool = env.i(8);
        let param = env.unknown();
        let func = env.function(vec![param, param], bool);

        let appl = env.apply(func);
        env.apply_next_parameter(appl, x);
        env.apply_next_parameter(appl, y);
        env.get_return_type(appl)
    };

    // return point == { x = 100, y = 200 };
    env.assign(comparison, return_);

    process(env, vec![])
}

#[test]
#[ignore]
fn bench() {
    let time = std::time::Instant::now();
    for _ in 0..50000 {
        defaulting();
        list();
        if_expr();
        field_inference();
        article_feature_complete_example_go();
        article_showcase_example_go();
        static_function_application();
        generic_function_application();
    }

    panic!("took {:#?}", time.elapsed());
}
