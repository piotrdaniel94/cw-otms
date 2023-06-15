#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, BankMsg, Addr, Response, StdResult, SubMsg, WasmMsg};
use cw2::set_contract_version;
use cw20::{Balance, Cw20ExecuteMsg};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, GetCountResponse, InstantiateMsg, QueryMsg, CreateMsg, ListResponse, DetailsResponse};
use crate::state::{State, STATE, MINIMAL_DONATION, GenericBalance, Escrow, ESCROWS, all_escrow_ids};

use self::query::{query_list, query_detail};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw-otms";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        count: msg.count,
        owner: info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;
    MINIMAL_DONATION.save(deps.storage, &msg.minimal_donation)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("count", msg.count.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Increment {} => execute::increment(deps),
        ExecuteMsg::Reset { count } => execute::reset(deps, info, count),
        ExecuteMsg:: Donate {} => execute::donate(deps, info),
        ExecuteMsg:: Withdraw{} => execute::withdraw(deps, env, info),
        
        ExecuteMsg:: Create(msg)=> {execute::execute_create(deps, msg, Balance::from(info.funds), &info.sender)},
        ExecuteMsg:: SetRecipient { id, recipient } => execute::execute_set_recipient(deps, env, info, id, recipient),
        ExecuteMsg:: TopUp {id} => execute::execute_top_up(deps, id, Balance::from(info.funds)),
        ExecuteMsg:: Apporve {id} => execute::execute_approve(deps, id, env, info),
        ExecuteMsg:: Refund { id } => execute::execute_refund(deps, env, info, id),
    }
}

pub mod execute {
    use super::*;

    pub fn increment(deps: DepsMut) -> Result<Response, ContractError> {
        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            state.count += 1;
            Ok(state)
        })?;

        Ok(Response::new().add_attribute("action", "increment"))
    }

    pub fn reset(deps: DepsMut, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            if info.sender != state.owner {
                return Err(ContractError::Unauthorized {});
            }
            state.count = count;
            Ok(state)
        })?;
        Ok(Response::new().add_attribute("action", "reset"))
    }

    pub fn donate(deps: DepsMut, info:MessageInfo)-> Result<Response, ContractError>{
        let state = STATE.load(deps.storage)?;
        let minimal_donation = MINIMAL_DONATION.load(deps.storage)?;
        
        if info.funds.iter().any(|coin|{
            coin.denom == minimal_donation.denom 
            // &&  coin.amount >= minimal_donation.amount
        }){
            let temp = State {
                count: state.count+1,
                owner: state.owner
            };
            STATE.save(deps.storage, &temp)?;
        }

        Ok(Response::new().add_attribute("action", "donate"))
    }

    pub fn withdraw(deps:DepsMut, env: Env, info: MessageInfo)-> Result<Response, ContractError>{
        let state = STATE.load(deps.storage)?;
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized{}); 
        }
        let balance = deps.querier.query_all_balances(&env.contract.address)?;
        let bank_msg = BankMsg::Send {
             to_address: info.sender.to_string(),
             amount: balance, 
        };

        let resp = Response::new()
        .add_message(bank_msg)
        .add_attribute("action", "withdraw")
        .add_attribute("sender", info.sender.as_str());

        Ok(resp)    
    }

    pub fn execute_create(
        deps: DepsMut,
        msg: CreateMsg,
        balance: Balance,
        sender: &Addr,
    ) -> Result<Response, ContractError> {
        if balance.is_empty(){
            return Err(ContractError::EmptyBalance{});
        }
        let mut cw20_whitelist = msg.addr_whitelist(deps.api)?;
        let escrow_balance = match balance {
            Balance::Native(balance) => GenericBalance {
                native: balance.0,
                cw20: vec![],
            },
            Balance::Cw20(token) => {
                // make sure the token sent is on the whitelist by default
                if !cw20_whitelist.iter().any(|t| t == &token.address) {
                    cw20_whitelist.push(token.address.clone())
                }
                GenericBalance {
                    native: vec![],
                    cw20: vec![token],
                }
            }
        };

        let recipient: Option<Addr> = msg.recipient.and_then(|addr|deps.api.addr_validate(&addr).ok());
        let escrow = Escrow {
            arbiter: deps.api.addr_validate(&msg.arbiter)?,
            recipient,
            source: sender.clone(),
            title: msg.title,
            description: msg.description,
            end_height: msg.end_height,
            end_time: msg.end_time,
            balance: escrow_balance,
            cw20_whitelist,
        };

        // try to store it, fail if the id was already in use
        ESCROWS.update(deps.storage, &msg.id, |existing| match existing {
            None => Ok(escrow),
            Some(_) => Err(ContractError::AlreadyInUse {}),
        })?;

        let res = Response::new().add_attributes(vec![("action", "create"), ("id", msg.id.as_str())]);
        Ok(res)
    }

    pub fn execute_set_recipient(
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        id: String,
        recipient: String,
    ) -> Result<Response, ContractError> {
        let mut escrow = ESCROWS.load(deps.storage, &id)?;
        if info.sender != escrow.arbiter {
            return Err(ContractError::Unauthorized {});
        }

        let recipient = deps.api.addr_validate(recipient.as_str())?;
        escrow.recipient = Some(recipient.clone());
        ESCROWS.save(deps.storage, &id, &escrow)?;

        Ok(Response::new().add_attributes(vec![
            ("action", "set_recipient"),
            ("id", id.as_str()),
            ("recipient", recipient.as_str()),
        ]))
    }

    pub fn execute_top_up(
        deps: DepsMut,
        id: String,
        balance: Balance,
    )-> Result<Response, ContractError>{
        if balance.is_empty(){
            return Err(ContractError::EmptyBalance {});
        }

        let mut escrow = ESCROWS.load(deps.storage, &id)?;
        if let Balance::Cw20(token) = &balance {
            //ensure token is on the whitelist
            if !escrow.cw20_whitelist.iter().any(|t|t == &token.address){
                return Err(ContractError::NotInWhitelist{});
            }
        }
        escrow.balance.add_tokens(balance);
        ESCROWS.save(deps.storage, &id, &escrow)?;

        let res = Response::new().add_attributes(vec![("action", "top_up"), ("id", id.as_str())]);
        Ok(res)
    }

    pub fn execute_approve(
        deps: DepsMut,
        id: String,
        env: Env,
        info: MessageInfo,
    ) -> Result<Response,ContractError> {
        let escrow = ESCROWS.load(deps.storage, &id)?;
        if info.sender != escrow.arbiter {
            return Err(ContractError::Unauthorized {});
        }
        if escrow.is_expired(&env){
            return Err(ContractError::Expired{});
        }

        let recipient = escrow.recipient.ok_or(ContractError::RecipientNotSet{})?;
        
        //delete the escrow
        ESCROWS.remove(deps.storage, &id);

        //send all tokens out
        let messages: Vec<SubMsg> = send_tokens(&recipient, &escrow.balance)?;

        Ok(Response::new()
        .add_attribute("action", "approve")
        .add_attribute("id", id)
        .add_attribute("to", recipient)
        .add_submessages(messages))
    }

    pub fn execute_refund(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        id: String,
    ) -> Result<Response, ContractError> {
        let escrow = ESCROWS.load(deps.storage, &id)?;
        if !escrow.is_expired(&env) || info.sender != escrow.arbiter {
            Err(ContractError::Unauthorized {})
        } else {
            //delete the escrow
            ESCROWS.remove(deps.storage, &id);

            //send all tokens out
            let messages = send_tokens(&escrow.source, &escrow.balance)?;
            Ok(Response::new()
                .add_attribute("action", "refund")
                .add_attribute("id", id)
                .add_attribute("to", escrow.source)
                .add_submessages(messages))
        }
    }
}

fn send_tokens(to: &Addr, balance: &GenericBalance) -> StdResult<Vec<SubMsg>> {
    let native_balance = &balance.native;
    let mut msgs: Vec<SubMsg> = if native_balance.is_empty(){
        vec![]
    } else {
        vec![SubMsg:: new(BankMsg:: Send { to_address: to.into(), amount: native_balance.to_vec() })]
    };

    let cw20_balance = &balance.cw20;
    let cw20_msgs: StdResult<Vec<_>> = cw20_balance
        .iter()
        .map(|c|{
            let msg = Cw20ExecuteMsg:: Transfer { recipient: to.into(), amount: c.amount };
            let exec = SubMsg::new(WasmMsg::Execute { contract_addr: c.address.to_string(), msg: to_binary(&msg)?, funds: vec![] });
            Ok(exec)
        })
        .collect();
    msgs.append(&mut cw20_msgs?);
    Ok(msgs)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => to_binary(&query::count(deps)?),
        QueryMsg:: List {} => to_binary(&query_list(deps)?),
        QueryMsg:: Details { id } => to_binary(&query_detail(deps, id)?),
    }
}

pub mod query {
    use cw20::Cw20Coin;

    use super::*;

    pub fn count(deps: Deps) -> StdResult<GetCountResponse> {
        let state = STATE.load(deps.storage)?;
        Ok(GetCountResponse { count: state.count })
    }

    pub fn query_list(deps: Deps) -> StdResult<ListResponse>{
        Ok(ListResponse{
            escrows: all_escrow_ids(deps.storage)?,
        })
    }

    pub fn query_detail(deps: Deps, id: String) -> StdResult<DetailsResponse> {
        let escrow = ESCROWS.load(deps.storage, &id)?;
        let cw20_whitelist = escrow.human_whitelist();
        let native_balance = escrow.balance.native;
        let cw20_balance: StdResult<Vec<_>> = escrow
            .balance
            .cw20
            .into_iter()
            .map(|token|{
                Ok(Cw20Coin{
                    address: token.address.into(),
                    amount: token.amount
                })
            })
            .collect();

        let recipient = escrow.recipient.map(|addr| addr.into_string());

        let detail = DetailsResponse{
            id,
            arbiter: escrow.arbiter.into(),
            recipient,
            title: escrow.title,
            description: escrow.description,
            end_heigth: escrow.end_height,
            end_time: escrow.end_time,
            cw20_balance: cw20_balance?,
            source: escrow.source.into(),
            native_balacne: native_balance,
            cw20_whitelist,
        };
        Ok(detail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coin, coins, from_binary, Addr, Empty};
    use cw_multi_test::{App, Contract, ContractWrapper, Executor};

    fn counting_contract() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(execute, instantiate, query);
        Box::new(contract)
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 , minimal_donation: coin(10, "atom")};
        let info = mock_info("creator", &coins(1000, "atom"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 , minimal_donation: coin(10, "atom")};
        let info = mock_info("creator", &coins(2, "atom"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "atom"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17, minimal_donation: coin(10, "atom") };
        let info = mock_info("creator", &coins(2, "atom"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "atom"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "atom"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }

    #[test]
    fn donate(){
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17, minimal_donation: coin(10, "atom") };
        let info = mock_info("creator", &coins(2, "atom"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let auth_info = mock_info("creator", &coins(2, "atom"));
        let msg = ExecuteMsg:: Donate {};
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg);

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn withdraw(){
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg { count: 17, minimal_donation: coin(10, "atom") };
        let info = mock_info("owner", &coins(100, "atom"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
            
        let auth_info = mock_info("sender", &coins(2, "atom"));
        let msg = ExecuteMsg:: Donate {};
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg);

        let auth_info = mock_info("owner", &coins(2, "atom"));
        let msg = ExecuteMsg:: Withdraw {};
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg);

        // assert_eq!(deps.as_mut().querier.query_all_balances("owner").unwrap(),
        //             coins(10, "atom"));
        assert_eq!(deps.as_mut().querier.query_all_balances("sender").unwrap(),
                    vec![]);
        assert_eq!(deps.as_mut().querier.query_all_balances(mock_env().contract.address).unwrap(),
                    vec![]);

    }

    #[test]
    fn withdraw2(){
        let owner = Addr::unchecked("owner");
        let sender = Addr::unchecked("sender");

        let mut app = App::new(|router, _api, storage| {
            router
                .bank
                .init_balance(storage, &sender, coins(10, "atom"))
                .unwrap();
        });

        let contract_id = app.store_code(counting_contract());

        let contract_addr = app
            .instantiate_contract(
                contract_id,
                owner.clone(),
                &InstantiateMsg {
                    count: 17, minimal_donation: coin(10, "atom")
                },
                &[],
                "Counting contract",
                None,
            )
            .unwrap();

        app.execute_contract(
            sender.clone(),
            contract_addr.clone(),
            &ExecuteMsg::Donate {},
            &coins(10, "atom"),
        )
        .unwrap();

        app.execute_contract(
            owner.clone(),
            contract_addr.clone(),
            &ExecuteMsg::Withdraw {},
            &[],
        )
        .unwrap();

        assert_eq!(
            app.wrap().query_all_balances(owner).unwrap(),
            coins(10, "atom")
        );
        assert_eq!(app.wrap().query_all_balances(sender).unwrap(), vec![]);
        assert_eq!(
            app.wrap().query_all_balances(contract_addr).unwrap(),
            vec![]
        );
    }

    

}
