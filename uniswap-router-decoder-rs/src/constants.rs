/// Constants and ABI definitions for Uniswap Universal Router
/// Using alloy-sol-types for type-safe ABI handling
use alloy_primitives::Address;
use alloy_sol_types::sol;

/// Mainnet contract addresses
pub struct Addresses;

impl Addresses {
    /// Permit2 contract address
    pub const PERMIT2: Address = Address::new([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x22, 0xD4, 0x73, 0x03, 0x0F, 0x11, 0x6d, 0xDE, 0xE9,
        0xF6, 0xB4, 0x3a, 0xC7, 0x8B, 0xA3,
    ]);

    /// Universal Router contract address
    pub const UNIVERSAL_ROUTER: Address = Address::new([
        0x66, 0xa9, 0x89, 0x3c, 0xC0, 0x7D, 0x91, 0xD9, 0x56, 0x44, 0xAE, 0xDD, 0x05, 0xD0,
        0x3f, 0x95, 0xe1, 0xdB, 0xA8, 0xAf,
    ]);
}

// Define Universal Router ABI using sol! macro
sol! {
    #[sol(rpc)]
    contract UniversalRouter {
        /// Execute commands without deadline
        function execute(bytes commands, bytes[] inputs) external payable;

        /// Execute commands with deadline
        function execute(bytes commands, bytes[] inputs, uint256 deadline) external payable;

        /// V3 swap callback
        function uniswapV3SwapCallback(int256 amount0Delta, int256 amount1Delta, bytes data) external;

        /// V4 unlock callback
        function unlockCallback(bytes data) external returns (bytes);

        /// Get message sender
        function msgSender() external view returns (address);

        /// Get pool manager
        function poolManager() external view returns (address);

        /// Custom errors
        error ExecutionFailed(uint256 commandIndex, bytes message);
        error TransactionDeadlinePassed();
        error InvalidCommandType(uint256 commandType);
        error InvalidAction(bytes4 action);
        error V2TooLittleReceived();
        error V2TooMuchRequested();
        error V3TooLittleReceived();
        error V3TooMuchRequested();
        error V3InvalidSwap();
        error V4TooLittleReceived(uint256 minAmountOutReceived, uint256 amountReceived);
        error V4TooMuchRequested(uint256 maxAmountInRequested, uint256 amountRequested);
        error BalanceTooLow();
        error InsufficientETH();
        error InvalidPath();
        error OnlyMintAllowed();
    }
}

// Permit2 contract ABI
sol! {
    #[sol(rpc)]
    contract Permit2 {
        struct PermitDetails {
            address token;
            uint160 amount;
            uint48 expiration;
            uint48 nonce;
        }

        struct PermitSingle {
            PermitDetails details;
            address spender;
            uint256 sigDeadline;
        }

        struct PermitBatch {
            PermitDetails[] details;
            address spender;
            uint256 sigDeadline;
        }

        function permit(
            address owner,
            PermitSingle permitSingle,
            bytes signature
        ) external;

        function permit(
            address owner,
            PermitBatch permitBatch,
            bytes signature
        ) external;

        function allowance(
            address user,
            address token,
            address spender
        ) external view returns (uint160 amount, uint48 expiration, uint48 nonce);

        error SignatureExpired(uint256 signatureDeadline);
        error InvalidNonce();
    }
}

// V4 Pool Manager ABI
sol! {
    #[sol(rpc)]
    contract PoolManager {
        struct PoolKey {
            address currency0;
            address currency1;
            uint24 fee;
            int24 tickSpacing;
            address hooks;
        }

        struct SwapParams {
            bool zeroForOne;
            int256 amountSpecified;
            uint160 sqrtPriceLimitX96;
        }

        struct ModifyLiquidityParams {
            int24 tickLower;
            int24 tickUpper;
            int256 liquidityDelta;
            bytes32 salt;
        }

        function initialize(PoolKey key, uint160 sqrtPriceX96) external returns (int24 tick);

        function swap(PoolKey key, SwapParams params, bytes hookData) external returns (int256 swapDelta);

        function modifyLiquidity(
            PoolKey key,
            ModifyLiquidityParams params,
            bytes hookData
        ) external returns (int256 callerDelta, int256 feesAccrued);

        function unlock(bytes data) external returns (bytes result);

        function settle() external payable returns (uint256);

        function take(address currency, address to, uint256 amount) external;

        error PoolNotInitialized();
        error CurrencyNotSettled();
        error ManagerLocked();
    }
}

// V4 Position Manager ABI
sol! {
    #[sol(rpc)]
    contract PositionManager {
        function initializePool(PoolManager.PoolKey key, uint160 sqrtPriceX96) external payable returns (int24);

        function modifyLiquidities(bytes unlockData, uint256 deadline) external payable;

        function modifyLiquiditiesWithoutUnlock(bytes actions, bytes[] params) external payable;

        function unlockCallback(bytes data) external returns (bytes);

        function multicall(bytes[] data) external payable returns (bytes[] results);

        function nextTokenId() external view returns (uint256);

        function getPositionLiquidity(uint256 tokenId) external view returns (uint128 liquidity);

        error DeadlinePassed(uint256 deadline);
        error DeltaNotNegative(address currency);
        error DeltaNotPositive(address currency);
    }
}

// Router command function signatures in separate contracts for proper namespacing
sol! {
    interface RouterCommands {
        // V2 Swaps
        function V2_SWAP_EXACT_IN(address recipient, uint256 amountIn, uint256 amountOutMin, address[] path, bool payerIsUser);
        function V2_SWAP_EXACT_OUT(address recipient, uint256 amountOut, uint256 amountInMax, address[] path, bool payerIsUser);

        // V3 Swaps
        function V3_SWAP_EXACT_IN(address recipient, uint256 amountIn, uint256 amountOutMin, bytes path, bool payerIsUser);
        function V3_SWAP_EXACT_OUT(address recipient, uint256 amountOut, uint256 amountInMax, bytes path, bool payerIsUser);

        // V4 Operations - simplified poolKey as struct components
        function V4_SWAP(bytes actions, bytes[] params);
        function V4_INITIALIZE_POOL(address currency0, address currency1, uint24 fee, int24 tickSpacing, address hooks, uint160 sqrtPriceX96);
        function V4_POSITION_MANAGER_CALL(bytes unlockData, uint256 deadline);

        // Token operations
        function WRAP_ETH(address recipient, uint256 amountMin);
        function UNWRAP_WETH(address recipient, uint256 amountMin);
        function SWEEP(address token, address recipient, uint256 amountMin);
        function TRANSFER(address token, address recipient, uint256 value);
        function PAY_PORTION(address token, address recipient, uint256 bips);

        // Permit2 operations - simplified to avoid nested struct issues
        function PERMIT2_PERMIT(address token, uint160 amount, uint48 expiration, uint48 nonce, address spender, uint256 sigDeadline, bytes signature);
        function PERMIT2_TRANSFER_FROM(address token, address recipient, uint256 amount);
    }
}

// V4 Actions function signatures
sol! {
    interface V4Actions {
        // Swap actions
        function SWAP_EXACT_IN_SINGLE(address currency0, address currency1, uint24 fee, int24 tickSpacing, address hooks, bool zeroForOne, uint128 amountIn, uint128 amountOutMinimum, uint160 sqrtPriceLimitX96, bytes hookData);
        function SWAP_EXACT_IN(address currency0, address currency1, uint24 fee, int24 tickSpacing, address hooks, bool zeroForOne, uint128 amountIn, uint128 amountOutMinimum, bytes pathKey, bytes hookData);
        function SWAP_EXACT_OUT_SINGLE(address currency0, address currency1, uint24 fee, int24 tickSpacing, address hooks, bool zeroForOne, uint128 amountOut, uint128 amountInMaximum, uint160 sqrtPriceLimitX96, bytes hookData);
        function SWAP_EXACT_OUT(address currency0, address currency1, uint24 fee, int24 tickSpacing, address hooks, bool zeroForOne, uint128 amountOut, uint128 amountInMaximum, bytes pathKey, bytes hookData);

        // Settlement actions
        function SETTLE(address currency, uint256 amount, bool payerIsUser);
        function SETTLE_ALL(address currency, uint256 maxAmount);
        function SETTLE_PAIR(address currency0, address currency1);

        // Take actions
        function TAKE(address currency, address recipient, uint256 amount);
        function TAKE_ALL(address currency, uint256 minAmount);
        function TAKE_PAIR(address currency0, address currency1, address to);
        function TAKE_PORTION(address currency, address recipient, uint256 bips);

        // Position actions
        function MINT_POSITION(address currency0, address currency1, uint24 fee, int24 tickSpacing, address hooks, int24 tickLower, int24 tickUpper, uint256 liquidity, uint128 amount0Max, uint128 amount1Max, address owner, bytes hookData);
        function CLOSE_CURRENCY(address currency);
        function CLEAR_OR_TAKE(address currency, uint256 amountMax);

        // Utility actions - different names to avoid conflicts
        function SWEEP_V4(address currency, address to);
        function WRAP_V4(address recipient, uint256 amount);
        function UNWRAP_V4(address recipient, uint256 amount);
    }
}

/// Permit2 EIP-712 domain data
pub fn permit2_domain_data() -> serde_json::Value {
    serde_json::json!({
        "name": "Permit2",
        "chainId": 1,
        "verifyingContract": format!("{:?}", Addresses::PERMIT2)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn test_addresses() {
        assert_eq!(
            Addresses::PERMIT2,
            address!("000000000022D473030F116dDEE9F6B43aC78BA3")
        );
        assert_eq!(
            Addresses::UNIVERSAL_ROUTER,
            address!("66a9893cC07D91D95644AEDD05D03f95e1dBA8Af")
        );
    }
}
