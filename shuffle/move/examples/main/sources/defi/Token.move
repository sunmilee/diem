module 0x1::Token {

    const INSUFFICIENT_BALANCE: u64 = 1;
    const BALANCE_OVERFLOW: u64 = 2;
    const SEND_TO_SELF: u64 = 3;

    struct Token<T>{
        amount: u64
    }

    // type marker
    struct TRO {}

    struct TRE {}

    struct Mint has key, store {
        total_supply: u64,
        name: vector<u8>,
        symbol: vector<u8>,
        mint_event: EventHandle<MintEvent>,
        burn_event: EventHandle<BurnEvent>,
    }

    struct Account has key, store {
        balance: u64,
        sent_event: EventHandle<SentEvent>,
        received_event: EventHandle<ReceivedEvent>,
    }

    public fun total_supply<T>(minter: address): u64 acquires Mint {
        borrow_global<Mint>(minter).total_supply
    }

    public fun balance(account: address): u64 acquires Account {
        borrow_global<Account>(account).balance
    }

    public fun withdraw<T>(x: &mut Token<T>, amount: u64): Token<T> {
        assert(x.value >= amount, Errors::limit_exceeded(INSUFFICIENT_BALANCE));
        x.value = x.value - amount;
        Token<T> { value: amount }
    }

    public fun deposit<T>(x: &mut Token<T>, other: Token<T>) {
        assert(x.value <= Limits::max_u64() - amount, Errors::limit_exceeded(BALANCE_OVERFLOW));
        let Token { value } = other;
        x.value = x.value + value;
    }
}
