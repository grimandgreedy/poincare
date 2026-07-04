//! Runtime environment (scope chain) tests.

use poincare_lang::Env;

#[test]
fn define_and_get() {
    let mut env: Env<i32> = Env::new();
    env.define("x", 1);
    assert_eq!(env.get("x"), Some(&1));
    assert_eq!(env.get("y"), None);
}

#[test]
fn rebinding_overwrites_in_current_scope() {
    let mut env: Env<i32> = Env::new();
    env.define("x", 1);
    env.define("x", 2);
    assert_eq!(env.get("x"), Some(&2));
}

#[test]
fn inner_scope_shadows_outer() {
    let mut env: Env<i32> = Env::new();
    env.define("x", 1);
    env.enter_scope();
    env.define("x", 99);
    assert_eq!(env.get("x"), Some(&99));
    env.exit_scope();
    assert_eq!(env.get("x"), Some(&1));
}

#[test]
fn get_searches_outer_scopes() {
    let mut env: Env<i32> = Env::new();
    env.define("outer", 5);
    env.enter_scope();
    assert_eq!(env.get("outer"), Some(&5));
    env.exit_scope();
}

#[test]
fn assign_updates_nearest_existing_binding() {
    let mut env: Env<i32> = Env::new();
    env.define("x", 1);
    env.enter_scope();
    assert!(env.assign("x", 42));
    // The outer binding was updated, not shadowed.
    assert_eq!(env.names_in_current_scope().count(), 0);
    env.exit_scope();
    assert_eq!(env.get("x"), Some(&42));
}

#[test]
fn assign_to_missing_name_fails() {
    let mut env: Env<i32> = Env::new();
    assert!(!env.assign("nope", 1));
}

#[test]
fn global_scope_is_never_popped() {
    let mut env: Env<i32> = Env::new();
    assert_eq!(env.depth(), 1);
    env.exit_scope();
    assert_eq!(env.depth(), 1);
}
