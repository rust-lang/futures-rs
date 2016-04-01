Weird compiler error messages

==============================================

repro: cargo build
why: wtf is this error

bad - 82cf968f59b1e1fe6c340c3cf91674c3e7a659d2
fix - aabf1aa0b5e1629d1f5fb7652e16bfc4d4f0d025

==============================================

repro: cargo build
why: wtf is this error

bad - 64cce07a7321df837caf264fe7d9713ed54bbb60
fix - 01dc3645ff71417825609add295fc2411f19c1b5

==============================================

repro: cargo test
why: good lord that is a lot of output, and the actual line in question is
     incredibly hard to find as there's so much noise and numbers and whatnot.
     First error is fixed with a fmt::Debug bound added

bad - 1fe8bf3457e1a841d87c5b73f6fd5d86e2752634
fix - 3d7c170846bf70214058d2d32bd04067b48b8288

==============================================

repro: cargo test
why: this is an absurdly long error message for the simple fix of "add `move`"

bad - 3d7c170846bf70214058d2d32bd04067b48b8288
fix - 0371b68b3f1987776d3837cac2b60102084c29ce

==============================================

repro: cargo test
why: difficult to understand error message, very long

bad - 45bfdee2c6fa8fef7c4a093b58de58dc86f444e8
fix - aa739da2bb0ec4ac512380de54ed093cb9788c78

==============================================

repro: cargo test
why: error message is... kinda inscrutable, fix is subtle, has API
     ramifications, and I'm not sure whether it should be necessary?

bad - ff25449c4f4e360dc219b1c0896df7593b33b05e
fix - bdcc2cefd039052df63f992361f1a12dcee30d2b


