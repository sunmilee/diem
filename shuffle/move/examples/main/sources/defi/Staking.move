module Sender::Staking {
    use Sender::TRO;
    use Sender::TRE;

    // Error codes
    /// StakingPool resource of the the two token pair already exists.
    const ESTAKING_POOL_PAIR: u64 = 0;

    // staking pool address can be stored globally - do a borrow global of two token type parameters
    struct GlobalStakingMetadata<StakingTokenType: store + drop, RewardTokenType: store + drop> has key, drop, store {}

    struct StakingPool<StakingTokenType: store + drop, RewardTokenType: store + drop> has key, drop, store {
//        stakedTokenTotal: u64,
        stakedTokens: Token<StakingTokenType>,
        apy: u64,
    }

    struct StakingMetadata<StakingTokenType: store + drop, RewardTokenType: store + drop> has key, drop, store {
        num_token_staked: u64,
        // last updated amount of rewards to be claimed
        num_reward_tokens: u64,
        // time since last reward claim. Resets on every claimReward()
        time: u64,
    }

    // publish stake pool resource under your account
    public(script) fun createStakingPool(account: &signer, stakingToken: Token, rewardToken: Token, apy: u64) {
        // abort a staking pool with the same pair already exists
        assert!(
            !exists<StakingPool<stakingToken, rewardToken>>(Signer::address_of(account)),
            Errors::already_published(ESTAKING_POOL_PAIR)
        );
        move_to(account, StakingPool<stakingToken, rewardToken>{
            stakedTokenTotal: 0,
            apy: apy,
        });
    }

    public(script) fun stakeToken(account: &signer, stakingToken: Token<StakingTokenType>, rewardToken: Token<RewardTokenType>, stakingPoolAddress: address) acquires StakingMetadata {
        if (!exists<StakingMetadata<stakingToken, rewardToken>>(Signer::address_of(account))) {
            move_to(account, StakingMetadata<stakingToken, rewardToken>{
                num_token_staked: 0,
                num_reward_tokens: 0,
                time: current_timestamp,
            });
        };
        let staking_metadata = &mut borrow_global<StakingMetadata<account>>(Signer::address_of(account));
        // calculate num reward token
        // update time
        *staking_metadata.num_token_staked = *staking_metadata.num_token_staked + amount;
        let staking_pool_tokens = borrow_global_mut(StakingPool<stakingToken, rewardToken>(address));
        Token::deposit(&mut staking_pool_tokens.stakedTokens, stakingToken);
    }

    public(script) fun unstakeToken(account: &signer, tokenA: tokenType, tokenB: tokenType, amount: u64, stakingPoolAddress: address) {
//        let token = Token{};
        // list to assert that person has permissio to release the token
        // only owner can release the 10 TRO
        // have a flag that says it's under my account but another address can unlock it


        // staking pool will transfer token A to you and change stake metadata struct under own account to have less deposit and get more reward
    }

    public(script) fun claimReward(account: signer) {
        borrow_global<StakingMetadata<type1, type2>>(account).amount
        transfer from staking pool to the account resource of token reward
    }
}

////    0xdeadbeef TRA-TRB, TRC-TRD parameteralized by types
////    vector of resource indexed by token combo
//
//- multiple staking pool
//- setup stake pool
//- want TRO for TRE at x exchange rate
//- publish resource under your account which would store TRE tokens
//- Minting procedure
//- XDX module
// staking pool metadata is stored under the sender's address


// storing token under author's account. only the child knows about the parent. child contains an ID it can use to find the parent
//