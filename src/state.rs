use cosmwasm_schema::cw_serde;

use cw20::{Balance, Cw20CoinVerified};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Coin, Env, Timestamp, StdResult, Order, Storage};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub count: i32,
    pub owner: Addr,
}

pub const STATE: Item<State> = Item::new("state");
pub const MINIMAL_DONATION: Item<Coin> = Item::new("minimal_donation");

#[cw_serde]
#[derive(Default)]
pub struct GenericBalance {
    pub native: Vec<Coin>,
    pub cw20: Vec<Cw20CoinVerified>,
}

impl GenericBalance {
    pub fn add_tokens(&mut self, add: Balance){
        match add {
           Balance::Native(balance) => {
                for token in balance.0{
                    let index = self.native.iter().enumerate().find_map(|(i, exist)|{
                        if exist.denom == token.denom{
                            Some(i)
                        } else {
                            None
                        }
                    });
                    match index {
                        Some(idx) => self.native[idx].amount += token.amount,
                        None => self.native.push(token),
                    }
                }
           }
           Balance::Cw20(token)=>{
                let index = self.cw20.iter().enumerate().find_map(|(i, exist)|{
                    if exist.address == token.address{
                        Some(i)
                    } else {
                        None
                    }
                });
                match index {
                    Some(idx)=> self.cw20[idx].amount += token.amount,
                    None => self.cw20.push(token),
                }
           }
        }
    }
}

#[cw_serde]
pub struct Escrow{
    //arbiter can decide to approve or refund the escrow
    pub arbiter: Addr, 

    //if apporve funds go to recipient, cannot approve if recipient is none
    pub recipient: Option<Addr>,

    //if refund, funds go to the source
    pub source: Addr,

    //title of escrow, for example for a bug bounty "Fix issue in contract.rs"
    pub title: String,

    //Description
    pub description: String,

    //when end height set and block height exceeds this value, the escrow is expired.
    //Once an escrow is expired, it can be returned to the original funder(via "refund").
    pub end_height: Option<u64>,

    // When end time (in seconds since epoch 00:00:00 UTC on 1 January 1970) is set and
    // block time exceeds this value, the escrow is expired.
    // Once an escrow is expired, it can be returned to the original funder (via "refund").
    pub end_time: Option<u64>,
    
    // Balance in Native and Cw20 tokens
    pub balance: GenericBalance,
    
    // All possible contracts that we accept tokens from
    pub cw20_whitelist: Vec<Addr>,
}

impl Escrow {
    pub fn is_expired(&self, env:&Env)->bool{
        if let Some(end_height) = self.end_height{
            if env.block.height > end_height{
                return true;
            }
        }
        if let Some(end_time) = self.end_time{
            if env.block.time > Timestamp::from_seconds(end_time){
                return true;
            }
        }

        false
    }

    pub fn human_whitelist(&self)->Vec<String> {
        self.cw20_whitelist.iter().map(|a|a.to_string()).collect()
    }
}

pub const ESCROWS: Map<&str, Escrow> = Map::new("escrow");

//This returns the list of ids for all registered escrows
pub fn all_escrow_ids(storage: &dyn Storage) -> StdResult<Vec<String>> {
    ESCROWS
        .keys(storage, None, None, Order::Ascending)
        .collect()
}