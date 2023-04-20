---
title: "Fungible Asset"
id: "fungible-asset"
---
import ThemedImage from '@theme/ThemedImage';
import useBaseUrl from '@docusaurus/useBaseUrl';

# Fungible Asset

Fungible assets (FA) are an essential part of the Aptos ecosystem, as they enable the creation and transfer of fungible units, which can represent different assets, such as currency, shares, material in games, or many other types of assets. Furthermore, fungible assets can be used to build decentralized applications that require a token economy, such as decentralized exchanges or gaming platforms.

The [fungible asset module](https://github.com/aptos-labs/aptos-core/blob/main/aptos-move/framework/aptos-framework/sources/fungible_asset.move) provides a standard, type-safe framework for defining fungible assets within the Aptos Move ecosystem.

In this standard, fungible assets are stored in `Object<FungibleStore>` that has a specific amount of units. Fungible assets are units that are interchangeable with others of the same metadata. The standard is built upon object model so all the resources defined here are included in object resource group and stored inside objects. The fungible assets within an object can be divided into smaller units creating new stores without creating new units. Similarly they can be combined to aggregate units into fewer objects. The standard also supports minting new units and burning existing units with appropriate controls.

The relationship between the structures laid out in this standard is shown in this diagram.
<div style={{textAlign: "center"}}>
<ThemedImage
alt="fungible asset architecture"
sources={{
    light: useBaseUrl('/img/docs/fungible-asset.svg'),
    dark: useBaseUrl('/img/docs/fungible-asset-dark.svg'),
  }}
/>
</div>

## Difference with Aptos Coin

FA is a broader category than just coins. While fungible coins are just one possible use case of FA, it can represent a wider range of fungible items, such as in-game assets like gems or rocks, event tickets, and partial ownership of real-world assets. FA is constructed using an object model, which provides the flexibility for customizable, detailed management and offers a new programming model based on objects.

Minimally, Aptos coin should be interchangeable with FA, it is up to the will of the ecosystem to dictate the whether or not Aptos coin is replaced by a FA equivalent.

## Structures

### Metadata Object

Within FA, metadata defines attributes of the type and other common features. The type, itself, is defined by the Object or address where this information lies. In other words, two assets with identical metadata but distinct Objects are not the same. The metadata layout is defined as:

```rust
#[resource_group_member(group = aptos_framework::object::ObjectGroup)]
struct Metadata has key {
    supply: Option<Supply>,
    /// Name of the fungible metadata, i.e., "USDT".
    name: String,
    /// Symbol of the fungible metadata, usually a shorter version of the name.
    /// For example, Singapore Dollar is SGD.
    symbol: String,
    /// Number of decimals used for display purposes.
    /// For example, if `decimals` equals `2`, a balance of `505` coins should
    /// be displayed to a user as `5.05` (`505 / 10 ** 2`).
    decimals: u8,
}
```

### Fungible Asset and Fungible Store

FA allows typing to be decoupled from metadata by allocating an object reference that points at the metadata. Hence a set of units of FA is represented as an amount and a reference to the metadata, as shown:

```rust
struct FungibleAsset {
    metadata: Object<Metadata>,
    amount: u64,
}
```

In contrast to Objects and addresses, a Coin uses a generic, or the `CoinType`, to support distinct typing within the Coin framework. For example, `Coin<A>` and `Coin<B>` are two distinct coins, if `A != B`.

The fungible assets is a struct representing the type and the amount of units held. As the struct does not have either key or store abilities, it can only be passed from one function to another but must be consumed by the end of a transaction. Specifically, it must be deposited back into a fungible store at the end of the transaction:

```rust
#[resource_group_member(group = aptos_framework::object::ObjectGroup)]
    struct FungibleStore has key {
    /// The address of the base metadata object.
    metadata: Object<Metadata>,
    /// The balance of the fungible metadata.
    balance: u64,
    /// Fungible Assets transferring is a common operation, this allows for freezing/unfreezing accounts.
    frozen: bool,
}
```

The only extra field added here is `frozen`. if it is `true`, this object is frozen, i.e. deposit and withdraw are both disabled without using `TransferRef` in the next section.

### References

Reference(ref) is the means to implement permission control across different standard in Aptos. In different contexts, it may be called capabilities sometimes. In FA standard, there are three distinct refs one each for minting, transferring, and burning FA respectively called `MintRef`, `TransferRef`, and `BurnRef`. Each ref contains a reference to the FA metadata:

```rust
struct MintRef has drop, store {
    metadata: Object<Metadata>
}

struct TransferRef has drop, store {
    metadata: Object<Metadata>
}

struct BurnRef has drop, store {
    metadata: Object<Metadata>
}
```

Ref owners can do the following operations depending on the refs they own:

- `MintRef` offers the capability to mint new FA units.
- `TransferRef` offers the capability to mutate the value of `freeze` in any `FungbibleStore` of the same metadata or transfer FA by ignoring `freeze`.
- `BurnRef` offers the capability to burn or delete FA units.

The three refs collectively act as the building blocks of various permission control system as they have `store` so can be passed around and stored anywhere. Please refer to the source file for `mint()`, `mint_to()`, `burn()`, `burn_from()`, `withdraw_with_ref()`, `deposit_with_ref()`, and `transfer_with_ref()`: These functions are used to mint, burn, withdraw, deposit, and transfer FA using the MintRef, BurnRef, and TransferRef.

Note, these are framework functions and must be combined with business logic to produce a usable system. Developers who want to use these functions should familiarize themselves with the concepts of Aptos object model and understand how the reference system enables extensible designs within Aptos move.

### Creators

A FA creator can add fungibility to any object at creation by taking `&ConstructorRef` with required information to
make that object a metadata of the associated FA. Then FA of this metadata can be minted and used.
new `CoinType`.

```rust
public fun add_fungibility(
    constructor_ref: &ConstructorRef,
    monitoring_supply_with_maximum: Option<Option<u128>>,
    name: String,
    symbol: String,
    decimals: u8,
): Object<Metadata>
```

The creator has the opportunity to define a name, symbol, decimals, and whether or not the total supply for the FA is
monitored. The following applies:

- The first three of the above (`name`, `symbol`, `decimals`)  are purely metadata and have no impact for onchain
  applications. Some applications may use decimal to equate a single Coin from fractional coin.
- Monitoring supply (`monitor_supply`) helps track total FA in supply. However, due to the way the parallel executor
  works, turning on this option will prevent any parallel execution of mint and burn. If the coin will be regularly
  minted or burned, consider disabling `monitor_supply`.

### Users

Coin users can:

- Merging two FAs of the same metadata object.
- Extracting FA from a fungible store into another.
- Ability to deposit and withdraw from a `FungibleStore` and emit events as a result.
- Allows for users to register a `CoinStore<CoinType>` in their account to handle coin.

### Primitives

At creation, the creator has the option to generate refs from the same `&ConstructorRef` to manage FA. These will need
to be stored in global storage to be used later.

#### Mint

If the manager would like to mint FA, they must retrieve a reference to `MintRef`, and call:

```rust
public fun mint(ref: &MintRef, amount: u64): FungibleAsset
```

This will produce a new FA of the metadata in the ref, containing a value as dictated by the `amount`. If supply is
tracked, then it will also be adjusted. There is also a `mint_to` function that also deposits to a `FungibleStore`
after minting as a helper.

#### Burn

The opposite operation of minting. Likewise, a reference to `BurnRef` is required and call:

```rust
public fun burn(ref: &BurnRef, fa: FungibleAsset)
```

This will reduce the passed-in `fa` to ashes as your will. There is also a `burn_from` function that forcibly withdraws
from an account first and then burn the fa withdrawn as a helper.

#### Transfer and Freeze/Unfreeze

`TransferRef` has two functions:

- Flip `frozen` in `FungibleStore` holding FA of the same metadata in the `TransferRef`. if
  it is false, the store is "frozen" that nobody can deposit to or withdraw from this store without using the ref.
- Withdraw from or deposit to a store ignoring `frozen` field.

To change `frozen`, call:

```rust
public fun set_frozen_flag<T: key>(
    ref: &TransferRef,
    store: Object<T>,
    frozen: bool,
)
```

:::tip
This function will emit a `FrozenEvent`.
:::

To forcibly withdraw, call:

```Rust
public fun withdraw_with_ref<T: key>(
    ref: &TransferRef,
    store: Object<T>,
    amount: u64
): FungibleAsset
```

:::tip
This function will emit a `WithdrawEvent`.
:::

To forcibly deposit, call

```rust
public fun deposit_with_ref<T: key>(
    ref: &TransferRef,
    store: Object<T>,
    fa: FungibleAsset
)
```

:::tip
This function will emit a `DepositEvent`.
:::

There is a function named `transfer_with_ref` that combining `withdraw_with_ref` and `deposit_with_ref` together as
a helper.

#### Merging Fungible Assets

Two FAs of the same type can be merged into a single struct that represents the accumulated value of the two  
independently by calling:

```rust
public fun merge(dst_fungible_asset: &mut FungibleAsset, src_fungible_asset: FungibleAsset)
```

After merging, `dst_fungible_asset` will have all the amounts.

#### Extracting Fungible Asset

A FA can have amount deducted to create another FA by calling:

```rust
public fun extract(fungible_asset:& mut FungibleAsset, amount: u64): FungibleAsset
```

:::tip
This function may produce FA with 0 amount, which is not usable. It is supposed to be merged with other FA or destroyed
through `destroy_zero()` in the module.
:::

#### Withdraw

The owner of a `FungibleStore` object that is not frozen can extract FA with a specified amount, by calling:

```rust
public fun withdraw<T: key>(owner: &signer, store: Object<T>, amount: u64): FungibleAsset
```

:::tip
This function will emit a `WithdrawEvent`.
:::

#### Deposit

Any entity can deposit FA into a `FungibleStore` object that is not frozen, by calling:

```rust
public fun deposit<T: key>(store: Object<T>, fa: FungibleAsset)
```

:::tip
This function will emit a `DepositEvent`.
:::

#### Transfer

The owner of a `FungibleStore` can directly transfer FA from that store to another if neither is frozen by calling:

```rust
public entry fun transfer<T: key>(sender: &signer, from: Object<T>, to: Object<T>, amount: u64)
```

:::tip
This will emit both `WithdrawEvent` and `DepositEvent` on the respective `Fungibletore`s.
:::

## Events

- `DepositEvent`: Emitted when fungible assets are deposited into a store.
- `WithdrawEvent`: Emitted when fungible assets are withdrawn from a store.
- `FrozenEvent`: Emitted when the frozen status of a fungible store is updated.

```rust
struct DepositEvent has drop, store {
    amount: u64,
}
```

```rust
struct WithdrawEvent has drop, store {
    amount: u64,
}
```

```rust
struct FrozenEvent has drop, store {
    frozen: bool,
}
```

# Primary `FungibleStore`

Each `FungibleStore` object has an owner. However, an owner may possess more than one store. When Alice sends FA to
Bob, how does she determine the correct destination? Additionally, what happens if Bob doesn't have a store yet?

To address these questions, the standard has been expanded to define primary and secondary stores.

- Each account owns only one undeletable primary store, the address of which is derived in a deterministic
  manner. from the account address and metadata object address. If primary store does not exist, it will be created if
  FA is going to be deposited by calling functions defined in `primary_fungible_store.move`
- Secondary stores do not have deterministic address and theoretically deletable. Users are able to create as many
  secondary stores as they want using the provided functions but they have to take care of the indexing by themselves.

The vast majority of users will have primary store as their only store for a specific type of fungible assets. It is
expected that secondary stores would be useful in complicated defi or other asset management contracts.

## How to enable Primary `FungibleStore`?

To add primary store support, when creating metadata object, instead of aforementioned `add_fungibility()`, creator
has to call:

```rust
public fun create_primary_store_enabled_fungible_asset(
    constructor_ref: &ConstructorRef,
    monitoring_supply_with_maximum: Option<Option<u128>>,
    name: String,
    symbol: String,
    decimals: u8,
)
```

The parameters are the same as those of `add_fungibility()`.

## Primitives

### Get Primary `FungibleStore`

To get the primary store object of a metadata object belonging to an account, call:

```rust
public fun primary_store<T: key>(owner: address, metadata: Object<T>): Object<FungibleStore>
```

:::tip
There are other utility functions. `primary_store_address` returns the deterministic address the primary store,
and `primary_store_exists` checks the existence, etc.
:::

### Manually Create Primary `FungibleStore`

If a primary store does not exist, any entity is able to create it by calling:

```rust
public fun create_primary_store<T: key>(owner_addr: address, metadata: Object<T>): Object<FungibleStore>
```

### Check Balance and Frozen Status

To check the balance of a primary store, call:

```rust
public fun balance<T: key>(account: address, metadata: Object<T>): u64
```

To check whether the given account's primary store is frozen, call

```rust
public fun is_frozen<T: key>(account: address, metadata: Object<T>): bool
```

### Withdraw

An owner can withdraw FA from their primary store by calling:

```rust
public fun withdraw<T: key>(owner: &signer, metadata: Object<T>, amount: u64): FungibleAsset
```

### Deposit

An owner can deposit FA to their primary store by calling:

```rust
public fun deposit(owner: address, fa: FungibleAsset)
```

### Transfer

An owner can deposit FA from their primary store to that of another account by calling:

```rust
public entry fun transfer<T: key>(sender: &signer, metadata: Object<T>, recipient: address, amount: u64)
```