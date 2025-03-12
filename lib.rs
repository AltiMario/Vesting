#![cfg_attr(not(feature = "std"), no_std, no_main)]

// Mark this module as an ink! smart contract
#[ink::contract]
mod vesting {
    use ink::prelude::vec::Vec;
    use ink::storage::Mapping;
    
    //----------------------------------
    // Error Handling
    //----------------------------------
    /// Custom error types for the vesting contract
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        ZeroAmount = 0,       // When trying to deposit 0 value
        NoFundsAvailable = 1, // When no funds are available for withdrawal
        TransferFailed = 2    // When token transfer fails
    }

    /// Type alias for Result that uses our custom Error
    pub type Result<T> = core::result::Result<T, Error>;

    //----------------------------------
    // Contract Storage
    //----------------------------------
    #[ink(storage)]
    pub struct Vesting {
        // Auto-incrementing ID for vesting schedules
        next_id: u64,
        // Mapping from schedule ID to vesting details
        schedules: Mapping<u64, VestingSchedule>,
        // Mapping from beneficiary to their schedule IDs
        beneficiary_to_ids: Mapping<AccountId, Vec<u64>>,
    }
    
    //----------------------------------
    // Default Implementation
    //----------------------------------
    /// Provides default initialization values for the contract
    impl Default for Vesting {
        fn default() -> Self {
            Self {
                next_id: 0,
                schedules: Mapping::new(),
                beneficiary_to_ids: Mapping::new(),
            }
        }
    }

    //----------------------------------
    // Vesting Schedule Structure
    //----------------------------------
    /// Represents a single vesting schedule
    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(
        feature = "std",
        derive(
            scale_info::TypeInfo,          // Required for metadata generation
            ink::storage::traits::StorageLayout // Required for storage mapping
        )
    )]
    struct VestingSchedule {
        owner: AccountId,       // Who created the vesting schedule
        beneficiary: AccountId, // Who can claim the funds
        amount: Balance,        // Amount to be vested
        unlock_time: Timestamp, // When funds become available
    }

    //----------------------------------
    // Core Contract Logic
    //----------------------------------
    impl Vesting {
        /// Constructor that initializes the contract
        #[ink(constructor)]
        pub fn new() -> Self {
            Self::default()
        }

        /// Deposit funds into a vesting schedule
        #[ink(message, payable)]
        pub fn deposit_fund(
            &mut self,
            beneficiary: AccountId,
            unlock_time: Timestamp,
        ) -> Result<()> {
            // Get the caller and transferred amount
            let owner = self.env().caller();
            let amount = self.env().transferred_value();
            
            // Prevent zero-value deposits
            if amount == 0 {
                return Err(Error::ZeroAmount);
            }

            // Generate new schedule ID with overflow check
            let id = self.next_id;
            self.next_id = id.checked_add(1).ok_or(Error::TransferFailed)?;

            // Create new vesting schedule
            let schedule = VestingSchedule {
                owner,
                beneficiary,
                amount,
                unlock_time,
            };

            // Store the schedule
            self.schedules.insert(id, &schedule);

            // Update beneficiary's schedule list
            let mut ids = self.beneficiary_to_ids.get(beneficiary).unwrap_or_default();
            ids.push(id);
            self.beneficiary_to_ids.insert(beneficiary, &ids);

            Ok(())
        }

        /// Withdraw all available vested funds for the caller
        #[ink(message)]
        pub fn withdraw_fund(&mut self) -> Result<()> {
            // Get caller and current block time
            let beneficiary = self.env().caller();
            let current_time: Timestamp = self.env().block_timestamp();

            // Retrieve all schedule IDs for beneficiary
            let ids = self.beneficiary_to_ids.get(beneficiary).unwrap_or_default();
            let mut total_amount: u128 = 0;
            let mut remaining_ids = Vec::new();

            // Process each schedule
            for &id in &ids {
                if let Some(schedule) = self.schedules.get(id) {
                    if schedule.unlock_time <= current_time {
                        // Add to total if unlocked, remove schedule
                        total_amount = total_amount.checked_add(schedule.amount)
                            .ok_or(Error::TransferFailed)?;
                        self.schedules.remove(id);
                    } else {
                        // Keep locked schedules
                        remaining_ids.push(id);
                    }
                }
            }

            // Check if any funds are available
            if total_amount == 0 {
                return Err(Error::NoFundsAvailable);
            }

            // Update remaining schedule IDs
            self.beneficiary_to_ids.insert(beneficiary, &remaining_ids);

            // Transfer funds to beneficiary
            self.env()
                .transfer(beneficiary, total_amount)
                .map_err(|_| Error::TransferFailed)?;

            Ok(())
        }
    }

}