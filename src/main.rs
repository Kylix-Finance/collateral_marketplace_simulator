use std::time::{SystemTime, UNIX_EPOCH};
/// This is a simple example of how to simulate a liquidation.
use std::{cmp::Ordering, collections::BTreeSet};

use egui_plot::{Bar, BarChart, Plot};

type BlockInded = u64;
type AccountId = u32;
type Balance = u32;

/// Represents a bid placed in the liquidation system.
#[derive(Clone, Eq, PartialEq, PartialOrd, Debug)]
pub struct Bid<AccountId, Balance, BlockNumber> {
    /// The account that placed the bid.
    pub bidder: AccountId,
    /// Amount the bidder is willing to spend in Bid Asset and the amount of the asset consumed so far.
    pub amount: Balance,
    /// Discount percentage offered by the bidder (1% to 100%).
    pub discount: u8,
    /// The block number when the bid was placed.
    pub blocknumber: BlockNumber,
    /// The sequential number of the transaction within the block.
    pub index: BlockInded,
    /// The original amount of the bid.
    pub original_amount: Balance,
    /// Current status of the bid.
    pub status: BidStatus,
}

impl<AccountId: Default, Balance: Default> Default for Liquidation<AccountId, Balance> {
    fn default() -> Self {
        Liquidation {
            account_liquidated: Default::default(),
            amount: Default::default(),
            status: LiquidationStatus::Untouched,
        }
    }
}

impl<
        AccountId: std::cmp::PartialOrd + std::cmp::Eq,
        Balance: std::cmp::PartialOrd + std::cmp::Eq,
        BlockNumber: std::cmp::Eq + std::cmp::PartialOrd,
    > Ord for Bid<AccountId, Balance, BlockNumber>
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.discount
            .cmp(&other.discount)
            .then(self.index.cmp(&other.index))
    }
}

/// Represents the possible statuses of a bid in the liquidation system.
#[derive(Clone, Eq, PartialEq, PartialOrd, Debug)]
pub enum BidStatus {
    /// The bid is currently active and available for fulfillment.
    Active,
    /// The bid has been partially fulfilled but not yet completed.
    PartiallyFilled,
    /// The bid has been fully fulfilled and is now closed.
    Fulfilled,
    /// The bid has been cancelled and is no longer available.
    Cancelled,
}

/// Represents a liquidation event that has occurred in the system.
#[derive(Clone, Eq, PartialEq, PartialOrd, Debug)]
pub struct Liquidation<AccountId, Balance> {
    /// The account that placed the bid.
    pub account_liquidated: AccountId,
    /// The account that placed the bid.
    pub amount: Balance,
    /// status of the liquidation
    pub status: LiquidationStatus,
}

/// Represents a liquidation event that has occurred in the system.
#[derive(Clone, Eq, PartialEq, PartialOrd, Debug)]
pub enum LiquidationStatus {
    /// The liquidation has been created
    Created,
    /// The liquidation has been partially fulfilled but not yet completed.
    PartiallyFilled,
    /// The liquidation has been fully fulfilled and is now closed.
    Fulfilled,
    /// The liquidation has been cancelled and is no longer available.
    Cancelled,
    /// The liquidation has been untouched
    Untouched,
}

type UserBid = Bid<AccountId, Balance, BlockInded>;
type SystemLiquidation = Liquidation<AccountId, Balance>;

#[derive(Default)]
struct LiquidationApp {
    // Bids in our liquidation system
    bids: BTreeSet<UserBid>,

    // The ongoing liquidation
    liquidation: SystemLiquidation,

    // Parameters that control how many new random bids to insert
    num_new_bids: u64,

    // Just to quickly view logs or status in the UI
    log_messages: Vec<String>,

    pub new_bid_amount: u32,
    pub new_bid_discount: u8,

    // must be sequential
    pub new_bid_index: BlockInded,

    pub discount_empties: std::collections::HashMap<u8, u32>,
}

impl eframe::App for LiquidationApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.heading("Liquidation Simulation - Kylix Finance");
        });

        // Side panel
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            // Create a single bid panel
            ui.group(|ui| {
                ui.label("Create a single Bids:");
                ui.label("Amount");
                ui.add(egui::DragValue::new(&mut self.new_bid_amount).speed(5));

                ui.label("Discount (%)");
                ui.add(egui::Slider::new(&mut self.new_bid_discount, 1..=20).text("discount"));

                ui.label("Index (block index)");
                ui.label(format!("Index (block index): {}", self.new_bid_index));

                if ui.button("Add a Bid").clicked() {
                    let new_bid = create_bid(
                        self.new_bid_amount,
                        self.new_bid_discount,
                        self.new_bid_index,
                    );
                    self.new_bid_index += 1;
                    self.bids.insert(new_bid);
                    self.log_messages.push(format!(
                        "Generated a new bid with amount: {}",
                        self.new_bid_amount
                    ));
                }
            });

            ui.separator();

            ui.group(|ui| {
                ui.label("Add Random Bids:");
                ui.add(egui::Slider::new(&mut self.num_new_bids, 1..=100).text("count"));
                if ui.button("Generate Bids").clicked() {
                    for _ in 0..self.num_new_bids {
                        let new_bid = create_random_bid(self.new_bid_index);
                        self.new_bid_index += 1;
                        self.bids.insert(new_bid);
                    }

                    self.log_messages
                        .push(format!("Generated {} new bids.", self.num_new_bids));
                }
            });

            ui.separator();

            ui.group(|ui| {
                ui.label("Liquidation Controls:");

                ui.horizontal(|ui| {
                    ui.label("Liquidation Account:");
                    ui.add(egui::DragValue::new(&mut self.liquidation.account_liquidated).speed(1));
                });

                ui.horizontal(|ui| {
                    ui.label("Liquidation Amount:");
                    ui.add(egui::DragValue::new(&mut self.liquidation.amount).speed(10));
                });

                if ui.button("Run Liquidation").clicked() {
                    let old_map = group_bids_by_discount(&self.bids);

                    // Call the liquidation function
                    liquidate(&mut self.bids, &mut self.liquidation);

                    let new_map = group_bids_by_discount(&self.bids);
                    for (discount, old_total) in old_map {
                        if old_total > 0 {
                            // If new_map doesn't have that discount or new total is zero => it got emptied
                            let new_total = new_map.get(&discount).copied().unwrap_or(0);
                            if new_total == 0 {
                                // increment the empties count for this discount
                                *self.discount_empties.entry(discount).or_insert(0) += 1;
                            }
                        }
                    }

                    self.log_messages.push(format!(
                        "Liquidation run. Amount left = {}, status = {:?}",
                        self.liquidation.amount, self.liquidation.status
                    ));
                }

                // Reset liquidation
                if ui.button("Reset Liquidation").clicked() {
                    self.liquidation = SystemLiquidation {
                        account_liquidated: 1,
                        amount: 5000,
                        status: LiquidationStatus::Created,
                    };
                    self.log_messages.push("Liquidation reset.".to_string());
                }
            });

            ui.separator();

            ui.heading("Liquidation");
            ui.label(format!("Current Status: {:?}", self.liquidation.status));
            ui.label(format!("Remaining Amount: {}", self.liquidation.amount));
            ui.label(format!(
                "Account Liquidated: {}",
                self.liquidation.account_liquidated
            ));

            ui.separator();

            ui.group(|ui| {
                ui.label("Logs:");
                // Just show the logs in descending order
                for msg in self.log_messages.iter().rev().take(10) {
                    ui.label(msg);
                }
            });
        });

        // Central panel

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Active Bids");
            {
                // Show the bar chart grouped by discount
                ui.heading("Bids by Discount");

                // Aggregate the bids by discount
                let discount_map = group_bids_by_discount(&self.bids);

                Plot::new("bids_by_discount")
                    .height(400.0) // set the height of the plot
                    .width(800.0) // set the width of the plot
                    .include_x(0.5)
                    .include_y(20.5)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .show(ui, |plot_ui| {
                        // Convert discount_map into a series of Bars
                        let mut bars = Vec::with_capacity(20);

                        for discount in 1..=20 {
                            // If no bids for this discount, total_amount = 0
                            let total_amount = discount_map.get(&discount).copied().unwrap_or(0);

                            // Bar::new(x_position, height)
                            let bar = Bar::new(discount as f64, total_amount as f64).width(0.6); // adjust for a nicer spacing
                            bars.push(bar);
                        }

                        let chart = BarChart::new(bars).name("Total Bids by Discount");

                        plot_ui.bar_chart(chart);
                        plot_ui.plot_bounds();
                    });

                ui.separator();

                ui.heading("Times Discounts Emptied");

                let mut bars_emptied = Vec::with_capacity(20);
                for discount in 1..=20 {
                    // If discount never had any empties, it’s 0
                    let empties_count = self.discount_empties.get(&discount).copied().unwrap_or(0);
                    let bar = Bar::new(discount as f64, empties_count as f64).width(0.6);
                    bars_emptied.push(bar);
                }

                Plot::new("discount_emptied_plot")
                    .height(200.0) // set the height of the plot
                    .width(800.0) // set the width of the plot
                    .include_x(0.5)
                    .include_y(20.5)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .show(ui, |plot_ui| {
                        let chart = BarChart::new(bars_emptied).name("Empties by Discount");
                        plot_ui.bar_chart(chart);
                    });

                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for bid in &self.bids {
                        ui.label(format!("{:?}", bid));
                    }
                });

                ui.separator();
            }
        });
    }
}

fn group_bids_by_discount(bids: &BTreeSet<UserBid>) -> std::collections::HashMap<u8, u32> {
    let mut discount_map = std::collections::HashMap::new();
    for bid in bids {
        *discount_map.entry(bid.discount).or_insert(0) += bid.amount;
    }
    discount_map
}

/// Entry point of the eframe/egui application
pub fn main() {
    let initial_app_state = LiquidationApp {
        // Start with some default liquidation
        liquidation: SystemLiquidation {
            account_liquidated: 1,
            amount: 5000,
            status: LiquidationStatus::Created,
        },
        num_new_bids: 3,
        new_bid_amount: 1000,
        discount_empties: std::collections::HashMap::new(),
        ..Default::default()
    };

    let native_options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "Kylix Finance - Collateral Liquidation Simulation",
        native_options,
        Box::new(|_cc| Ok(Box::new(initial_app_state))),
    );
}

// create a random bid with random values
fn create_random_bid(index: BlockInded) -> UserBid {
    // create a random amount between 100 and 10000
    let amount = (1 + rand::random::<u32>() % 99) * 100;
    // create a random discount between 0 and 20, multiple of 2
    let discount = rand::random::<u8>() % 10 * 2;
    create_bid(amount, discount, index)
}

fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u64
}

// create a bid with a specific amount and discount
fn create_bid(amount: u32, discount: u8, index: BlockInded) -> UserBid {
    UserBid {
        bidder: 1, // ALICE!
        amount,
        discount,
        blocknumber: get_timestamp(),
        index,
        status: BidStatus::Active,
        original_amount: amount,
    }
}

// let's create a function to liquidate some bids
fn liquidate(bids: &mut BTreeSet<UserBid>, liquidation: &mut SystemLiquidation) {
    if liquidation.amount == 0 {
        liquidation.status = LiquidationStatus::Fulfilled;
        return;
    }

    if bids.is_empty() {
        liquidation.status = LiquidationStatus::Untouched;
        return;
    }

    let mut remaining_amount = liquidation.amount;

    while remaining_amount > 0 {
        let Some(current_bid) = bids.pop_first() else {
            // No more bids to liquidate!
            break;
        };

        println!(
            "*** bid amount: {:?}, remaining amount: {:?}",
            current_bid.amount, remaining_amount
        );

        if current_bid.amount <= remaining_amount {
            remaining_amount -= current_bid.amount;
            println!("Liquidating bid: {:?}", remaining_amount);
        } else {
            // partial fill
            println!("Partial liquidating bid: {:?}", remaining_amount);
            let partially_filled_bid = Bid {
                bidder: current_bid.bidder,
                amount: current_bid.amount - remaining_amount,
                discount: current_bid.discount,
                blocknumber: current_bid.blocknumber,
                index: current_bid.index,
                original_amount: current_bid.original_amount,
                status: BidStatus::PartiallyFilled,
            };
            bids.insert(partially_filled_bid);
            remaining_amount = 0;
        }
    }
    liquidation.amount = remaining_amount;
    liquidation.status = if remaining_amount > 0 {
        LiquidationStatus::PartiallyFilled
    } else {
        LiquidationStatus::Fulfilled
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;
    use std::collections::BTreeSet;

    const ALICE: AccountId = 1;
    const BOB: AccountId = 2;
    const CHARLIE: AccountId = 3;

    #[test]
    fn just_a_simulation() {
        println!("🌊 Liquidation simulation 🌊");

        let mut bids: BTreeSet<UserBid> = BTreeSet::new();
        generate_random_bids(&mut bids, 3);

        log_debug(&bids);

        let mut liquidation = SystemLiquidation {
            account_liquidated: 1,
            amount: 5000,
            status: LiquidationStatus::Created,
        };

        println!("Liquidation: {:?}", liquidation);

        // liquidate some bids
        liquidate(&mut bids, &mut liquidation);

        print!("After liquidation: \n");
        println!("Liquidation: {:?}", liquidation);

        log_debug(&bids);
    }

    #[test]
    fn test_liquidate_zero_amount() {
        let mut bids = BTreeSet::new();

        bids.insert(Bid {
            bidder: ALICE,
            amount: 100,
            discount: 10,
            blocknumber: 1,
            index: 1,
            original_amount: 100,
            status: BidStatus::Active,
        });

        let mut liquidation = SystemLiquidation {
            amount: 0,
            status: LiquidationStatus::Created,
            account_liquidated: ALICE,
        };

        liquidate(&mut bids, &mut liquidation);

        assert_eq!(bids.len(), 1, "No bids should be removed if amount is 0");
        assert_eq!(
            liquidation.status,
            LiquidationStatus::Fulfilled,
            "If there's nothing to liquidate, status should be Fulfilled"
        );
    }

    #[test]
    fn test_liquidate_empty_bids() {
        let mut bids = BTreeSet::new();
        let mut liquidation = SystemLiquidation {
            amount: 1000,
            status: LiquidationStatus::Created,
            account_liquidated: ALICE,
        };

        liquidate(&mut bids, &mut liquidation);

        assert_eq!(liquidation.status, LiquidationStatus::Untouched);
        assert_eq!(liquidation.amount, 1000);
    }

    #[test]
    fn test_liquidate_single_bid_exact_amount() {
        let mut bids = BTreeSet::new();
        bids.insert(Bid {
            bidder: BOB,
            amount: 1000,
            discount: 10,
            blocknumber: 1,
            index: 1,
            original_amount: 1000,
            status: BidStatus::Active,
        });

        let mut liquidation = SystemLiquidation {
            amount: 1000,
            status: LiquidationStatus::Created,
            account_liquidated: ALICE,
        };

        liquidate(&mut bids, &mut liquidation);

        assert_eq!(bids.len(), 0);
        assert_eq!(liquidation.status, LiquidationStatus::Fulfilled);
        assert_eq!(liquidation.amount, 0);
    }

    #[test]
    fn test_liquidate_multiple_bids_with_remainder() {
        let mut bids = BTreeSet::new();

        // Add bids with different discounts
        bids.insert(Bid {
            bidder: BOB,
            amount: 500,
            discount: 15,
            blocknumber: 1,
            index: 1,
            original_amount: 500,
            status: BidStatus::Active,
        });

        bids.insert(Bid {
            bidder: CHARLIE,
            amount: 300,
            discount: 10,
            blocknumber: 1,
            index: 2,
            original_amount: 300,
            status: BidStatus::Active,
        });

        let mut liquidation = SystemLiquidation {
            amount: 1000,
            status: LiquidationStatus::Created,
            account_liquidated: ALICE,
        };

        liquidate(&mut bids, &mut liquidation);

        assert_eq!(liquidation.status, LiquidationStatus::PartiallyFilled);
        assert_eq!(liquidation.amount, 200);
    }

    #[test]
    fn test_partial_bid_fill() {
        let mut bids = BTreeSet::new();
        bids.insert(Bid {
            bidder: BOB,
            amount: 1500,
            discount: 10,
            blocknumber: 1,
            index: 1,
            original_amount: 1500,
            status: BidStatus::Active,
        });

        let mut liquidation = SystemLiquidation {
            amount: 1000,
            status: LiquidationStatus::Created,
            account_liquidated: ALICE,
        };

        liquidate(&mut bids, &mut liquidation);

        assert_eq!(bids.len(), 1);
        let remaining_bid = bids.iter().next().unwrap();
        assert_eq!(remaining_bid.amount, 500);
        assert_eq!(remaining_bid.status, BidStatus::PartiallyFilled);
        assert_eq!(liquidation.status, LiquidationStatus::Fulfilled);
        assert_eq!(liquidation.amount, 0);
    }

    #[test]
    fn test_bid_ordering() {
        let mut bids = BTreeSet::new();

        // Add bids with different discounts and indices
        bids.insert(Bid {
            bidder: BOB,
            amount: 100,
            discount: 20,
            blocknumber: 1,
            index: 2,
            original_amount: 100,
            status: BidStatus::Active,
        });

        bids.insert(Bid {
            bidder: CHARLIE,
            amount: 100,
            discount: 10,
            blocknumber: 1,
            index: 1,
            original_amount: 100,
            status: BidStatus::Active,
        });

        // Lower discount should be processed first
        let first_bid = bids.iter().next().unwrap();
        assert_eq!(first_bid.discount, 10);
        assert_eq!(first_bid.bidder, CHARLIE);
    }

    #[test]
    fn test_multiple_bids_exact_amount() {
        let mut bids = BTreeSet::new();

        bids.insert(Bid {
            bidder: BOB,
            amount: 600,
            discount: 15,
            blocknumber: 1,
            index: 1,
            original_amount: 600,
            status: BidStatus::Active,
        });

        bids.insert(Bid {
            bidder: CHARLIE,
            amount: 400,
            discount: 10,
            blocknumber: 1,
            index: 2,
            original_amount: 400,
            status: BidStatus::Active,
        });

        let mut liquidation = SystemLiquidation {
            amount: 1000,
            status: LiquidationStatus::Created,
            account_liquidated: ALICE,
        };

        liquidate(&mut bids, &mut liquidation);

        assert_eq!(bids.len(), 0);
        assert_eq!(liquidation.status, LiquidationStatus::Fulfilled);
        assert_eq!(liquidation.amount, 0);
    }

    /// helpers

    fn log_debug(bids: &BTreeSet<Bid<AccountId, Balance, BlockInded>>) {
        for bid in bids.iter() {
            println!("{:?}", bid);
        }
    }

    fn generate_random_bids(bids: &mut BTreeSet<Bid<AccountId, Balance, BlockInded>>, n: u64) {
        // inser 10 random bids
        for i in 0..n {
            bids.insert(create_random_bid(i));
        }
    }
}
