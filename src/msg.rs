use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Api, Addr, StdResult};
use cw20::{Cw20Coin, Cw20ReceiveMsg};

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
    
    //set the recipient of the given escrow
    SetRecipient{
        id: String,
        recipient: String,
    },

    TopUp{
        id: String,
    },

    //Approve sends all tokens to the recipient. Only the arbiter can do this
    Approve{
        id: String,
    },

    //Refund returns all remaining tokens to the original sender,
    //arbiter can do this anytime or anyone can do this after a timeout
    Refund{
        id: String,
    },

    //This accepts a properly-encoded ReceiveMsg from a cw20 contract
    Receive(Cw20ReceiveMsg),
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    #[returns(GetCountResponse)]
    GetCount {},

    #[returns(ListResponse)]
    List{},

    #[returns(DetailsResponse)]
    Details{id: String},
}

// We define a custom struct for each query response
#[cw_serde]
pub struct GetCountResponse {
    pub count: i32,
}

#[cw_serde]
pub struct ListResponse {
    //list all registered ids
    pub escrows: Vec<String>,
}

#[cw_serde]
pub struct DetailsResponse{
    //id of this escrow
    pub id: String,
    pub arbiter: String,
    //if approved the funds goto the recipient
    pub recipient: Option<String>,
    //if refunded, funds go to the source
    pub source: String,
    pub title: String,
    pub description: String,
    //when end height set and block height exceeds this value the escrow is expired.
    //Once a escrow is expired, it can be returned to the original funder(via "refund").
    pub end_height: Option<u64>,
    //block time exceeds this value the escrow is expired. 
    //once an escrow is expired, it can be returned to the original funder(via "refund").
    pub end_time: Option<u64>,
    //Balance in native tokens
    pub native_balance: Vec<Coin>,
    //Balance in cw20 tokens
    pub cw20_balance: Vec<Cw20Coin>,
    //whitelisted cw20 tokens
    pub cw20_whitelist: Vec<String>,
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

#[cw_serde]
pub enum ReceiveMsg {
    Create(CreateMsg),
    /// Adds all sent native tokens to the contract
    TopUp {
        id: String,
    },
}


