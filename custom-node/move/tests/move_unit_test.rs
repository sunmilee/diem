use move_unit_test::UnitTestingConfig;
use diem_framework::diem_framework_named_addresses;
use diem_vm::natives::diem_natives;

move_unit_test::register_move_unit_tests!(
    UnitTestingConfig::default_with_bound(Some(100_000))
    .with_named_addresses(diem_framework_named_addresses()),
    ".",
    r"src/.*\.move$",
    &move_stdlib::move_stdlib_modules_full_path(),
    Some(diem_natives())
);
