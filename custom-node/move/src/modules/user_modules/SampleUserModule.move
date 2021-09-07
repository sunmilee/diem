module 0x24163AFCC6E33B0A9473852E18327FA9::SampleUserModule {
    struct UserCoin has key, store {
        value: u64,
    }

    const COIN_ALREADY_EXISTS: u64 = 0;
    const COIN_DOES_NOT_EXIST: u64 = 1;
    const COIN_HAS_WRONG_VALUE: u64 = 2;

    public(script) fun mint_user_coin(owner: signer, amount: u64) {
    move_to(&owner, UserCoin { value: amount } );
    }

    #[test(owner = @0xA)]
    public(script) fun test_mint_coin(owner: signer) acquires UserCoin {
    // Before publishing, there is no `UserCoin` resource under the address.
    assert(!exists<UserCoin>(@0xA), COIN_ALREADY_EXISTS);

    mint_user_coin(owner, 42);

    // Check that there is a `UserCoin` resource published with the correct value.
    assert(exists<UserCoin>(@0xA), COIN_DOES_NOT_EXIST);
    assert(borrow_global<UserCoin>(@0xA).value == 42, COIN_HAS_WRONG_VALUE);
    }
}
