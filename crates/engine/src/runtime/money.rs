//! Money (SRD 5.1): the coins are items worth some copper pieces (`ItemDef::coin_cp`: cp 1,
//! sp 10, ep 50, gp 100, pp 1000). A shop whose currency is a coin counts every coin the hero
//! carries (the purse, in cp), takes payment from them and gives change in the fewest gold, silver
//! and copper pieces; scripts read and move money in cp. Shops in another currency (projects of
//! 0.30 and older) count that one item as before.

use super::Game;

impl Game {
    /// The project's coins: (item id, worth in cp), the most valuable first.
    pub(super) fn coin_kinds(&self) -> Vec<(String, u32)> {
        let mut v: Vec<(String, u32)> = self.project.db.items.iter().filter(|d| d.coin_cp > 0).map(|d| (d.id.clone(), d.coin_cp)).collect();
        v.sort_by_key(|k| std::cmp::Reverse(k.1));
        v
    }

    /// What one of a shop's currency is worth in cp (0 = not a coin: the old single-item money).
    pub(super) fn currency_worth(&self, currency: &str) -> u32 {
        self.project.db.item(currency).map_or(0, |d| d.coin_cp)
    }

    /// The hero's money in copper pieces (every coin carried).
    pub fn money_cp(&self) -> u32 {
        self.world.money_cp()
    }

    /// Pays `cp` from the hero's coins (change back); false (nothing taken) when short.
    pub fn pay_cp(&mut self, cp: u32) -> bool {
        self.world.pay_cp(cp)
    }

    /// Gives the hero `cp` in the fewest coins; returns what didn't fit (in cp).
    pub fn give_cp(&mut self, cp: u32) -> u32 {
        self.world.give_cp(cp)
    }

    /// A sum of copper pieces as it reads, in the player's language (`1 gp 5 sp`).
    pub fn format_cp(cp: u32) -> String {
        crate::srd::equipment::format_cp(cp, [&crate::lang::tr_ui("gp"), &crate::lang::tr_ui("sp"), &crate::lang::tr_ui("cp")])
    }

    /// `money=` for status: the purse in cp (`-` without coins).
    pub(super) fn money_status(&self) -> String {
        if self.coin_kinds().is_empty() {
            "-".into()
        } else {
            self.money_cp().to_string()
        }
    }
}
