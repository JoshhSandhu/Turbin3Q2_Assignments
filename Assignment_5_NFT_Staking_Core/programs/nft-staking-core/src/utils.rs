use anchor_lang::prelude::*;
use anchor_spl::token_interface::{mint_to_checked, Mint, MintToChecked, TokenAccount, TokenInterface};
use mpl_core::{
    accounts::{BaseAssetV1, BaseCollectionV1},
    fetch_plugin,
    instructions::{AddCollectionPluginV1CpiBuilder, UpdateCollectionPluginV1CpiBuilder},
    types::{Attribute, Attributes, Plugin, PluginAuthority, PluginType},
};

use crate::errors::StakingError;
use crate::state::Config;

pub const SECONDS_PER_DAY: i64 = 86400;

pub const STAKED: &str = "staked";
pub const STAKED_AT: &str = "staked_at";
pub const LAST_CLAIMED_AT: &str = "last_claimed_at";
pub const TOTAL_STAKED: &str = "total_staked";

pub fn attribute(key: &str, value: &str) -> Attribute {
    Attribute {
        key: key.to_string(),
        value: value.to_string(),
    }
}

pub fn get_attribute<'a>(list: &'a [Attribute], key: &str) -> Option<&'a str> {
    list.iter().find(|a| a.key == key).map(|a| a.value.as_str())
}

pub fn set_attribute(list: &mut Vec<Attribute>, key: &str, value: &str) {
    match list.iter_mut().find(|a| a.key == key) {
        Some(a) => a.value = value.to_string(),
        None => list.push(attribute(key, value)),
    }
}

pub fn get_timestamp(list: &[Attribute], key: &str) -> Result<i64> {
    let value = get_attribute(list, key).ok_or(StakingError::InvalidTimestamp)?;
    Ok(value.parse::<i64>().map_err(|_| StakingError::InvalidTimestamp)?)
}

pub fn staked_attributes(nft: &AccountInfo) -> Result<Vec<Attribute>> {
    let list = match fetch_plugin::<BaseAssetV1, Attributes>(nft, PluginType::Attributes) {
        Ok((_, attributes, _)) => attributes.attribute_list,
        Err(_) => return Err(StakingError::NotStaked.into()),
    };
    require!(get_attribute(&list, STAKED) == Some("true"), StakingError::NotStaked);
    Ok(list)
}

pub fn last_claimed_at(list: &[Attribute]) -> Result<i64> {
    match get_timestamp(list, LAST_CLAIMED_AT) {
        Ok(ts) => Ok(ts),
        Err(_) => get_timestamp(list, STAKED_AT),
    }
}

pub fn full_days(from: i64, now: i64) -> Result<i64> {
    let elapsed = now.checked_sub(from).ok_or(StakingError::InvalidTimestamp)?;
    require!(elapsed >= 0, StakingError::InvalidTimestamp);
    Ok(elapsed / SECONDS_PER_DAY)
}

pub fn reward_for(days: i64, points_per_stake: u32) -> Result<u64> {
    Ok((days as u64)
        .checked_mul(points_per_stake as u64)
        .ok_or(StakingError::Overflow)?)
}

pub fn mint_rewards<'info>(
    token_program: &Interface<'info, TokenInterface>,
    rewards_mint: &InterfaceAccount<'info, Mint>,
    user_rewards_ata: &InterfaceAccount<'info, TokenAccount>,
    config: &Account<'info, Config>,
    collection: Pubkey,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let seeds: &[&[u8]] = &[b"config", collection.as_ref(), &[config.config_bump]];
    let accounts = MintToChecked {
        mint: rewards_mint.to_account_info(),
        to: user_rewards_ata.to_account_info(),
        authority: config.to_account_info(),
    };
    mint_to_checked(
        CpiContext::new_with_signer(token_program.to_account_info(), accounts, &[seeds]),
        amount,
        rewards_mint.decimals,
    )
}

pub fn update_total_staked<'info>(
    mpl_core_program: &AccountInfo<'info>,
    collection: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    update_authority: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    signer_seeds: &[&[u8]],
    increment: bool,
) -> Result<()> {
    let existing = fetch_plugin::<BaseCollectionV1, Attributes>(collection, PluginType::Attributes).ok();
    let mut list = match &existing {
        Some((_, attributes, _)) => attributes.attribute_list.clone(),
        None => Vec::new(),
    };

    let current = match get_attribute(&list, TOTAL_STAKED) {
        Some(value) => value.parse::<u64>().map_err(|_| StakingError::InvalidCounter)?,
        None => 0,
    };
    let next = if increment {
        current.checked_add(1)
    } else {
        current.checked_sub(1)
    }
    .ok_or(StakingError::InvalidCounter)?;
    set_attribute(&mut list, TOTAL_STAKED, &next.to_string());

    let plugin = Plugin::Attributes(Attributes { attribute_list: list });
    if existing.is_some() {
        UpdateCollectionPluginV1CpiBuilder::new(mpl_core_program)
            .collection(collection)
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(plugin)
            .invoke_signed(&[signer_seeds])?;
    } else {
        AddCollectionPluginV1CpiBuilder::new(mpl_core_program)
            .collection(collection)
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(plugin)
            .init_authority(PluginAuthority::UpdateAuthority)
            .invoke_signed(&[signer_seeds])?;
    }
    Ok(())
}
