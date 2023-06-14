use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Api, Addr, StdResult};

#[cw_serde]
pub struct InstantiateMsg {
    pub count: i32,
    pub minimal_donation: Coin,
}

#[cw_serde]
pub enum ExecuteMsg {
    Increment {},
    Reset { count: i32 },
    Donate {},
    Withdraw{},

    Create(CreateMsg),
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    #[returns(GetCountResponse)]
    GetCount {},
}

// We define a custom struct for each query response
#[cw_serde]
pub struct GetCountResponse {
    pub count: i32,
}

#[cw_serde]
pub struct CreateMsg {
    //id is a human-readable name for the escrow to use later
    pub id: String,
    
    //arbiter can decide to approve or refund the escrow
    pub arbiter: String,
    
    //if approved, funds go to recipient
    pub recipient: Option<String>,

    //Title of escrow
    pub title: String,

    //Longer description of the escrow, e.g what conditions should be met 
    pub description: String,

    //When end height set and block height exceeds this value, the escrow is expired
    //Once an escrow is expired, it can be returned to the original funder(via "refund").
    pub end_height: Option<u64>,

    //When end time (in seconds since epoch 00:00:00 UTC on 1 January 1970) is set and 
    //block time exceeds this value, the escrow is expired.
    //Once an escrow is expired, it can be returned to the original funder (via "refund").
    pub end_time: Option<u64>,

    //Besides any possible tokens sent with the createMsg, this is a list of all cw20 token addresses
    //that are accepted by the escrow during a top-up.This is required to avoid a DoS attack by topping-up
    //with an invalid cw20 contract.
    pub cw20_whitelist: Option<Vec<String>>,
}

impl CreateMsg {
    pub fn addr_whitelist(&self, api: &dyn Api)->StdResult<Vec<Addr>>{
        match self.cw20_whitelist.as_ref(){
            Some(v) => v.iter().map(|h|api.addr_validate(h)).collect(),
            None => Ok(vec![]),
        }
    }
}