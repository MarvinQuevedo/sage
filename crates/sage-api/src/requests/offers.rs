use serde::{Deserialize, Serialize};

use crate::{
    Amount, CoinSpendJson, OfferRecord, OfferRecordStatus, OfferSummary, SpendBundleJson,
    TransactionSummary,
};

use super::TransactionResponse;

/// Create a new offer
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Create a new offer for peer-to-peer trading of assets."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MakeOffer {
    /// Assets requested in the offer
    pub requested_assets: Vec<OfferAmount>,
    /// Assets offered in exchange
    pub offered_assets: Vec<OfferAmount>,
    /// Transaction fee
    pub fee: Amount,
    /// Optional receive address
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(nullable = true))]
    pub receive_address: Option<String>,
    /// Optional expiration timestamp
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(nullable = true))]
    pub expires_at_second: Option<u64>,
    /// Whether to automatically import the offer
    #[serde(default = "yes")]
    #[cfg_attr(feature = "openapi", schema(default = true))]
    pub auto_import: bool,
    /// Optional specific coin IDs to use for the offer instead of auto-selecting
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(nullable = true))]
    pub coin_ids: Option<Vec<String>>,
}

/// Asset amount in an offer
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct OfferAmount {
    /// Optional asset ID (null for XCH)
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(nullable = true))]
    pub asset_id: Option<String>,
    /// Optional hidden puzzle hash for privacy
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(nullable = true))]
    pub hidden_puzzle_hash: Option<String>,
    /// Amount of the asset
    pub amount: Amount,
}

/// Response with created offer
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MakeOfferResponse {
    /// Offer string (bech32 encoded)
    pub offer: String,
    /// Offer ID
    pub offer_id: String,
}

/// Build the offer's coin spends without signing them.
///
/// Same inputs as [`MakeOffer`], but instead of signing in-process and
/// returning an encoded offer, it returns the unsigned coin spends so an
/// external signer (e.g. a Tangem card, which spends the
/// `p2_delegated_conditions` / "arbor" puzzle) can produce the BLS
/// signatures. Pair with `required_signatures` to get the messages to sign,
/// then `encode_offer` to assemble and encode the finished offer.
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Build the coin spends for a new offer without signing them, for external/hardware signers such as Tangem cards. Use `required_signatures` for the messages to sign, then `encode_offer` to assemble the encoded offer."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MakeOfferUnsigned {
    /// Assets requested in the offer
    pub requested_assets: Vec<OfferAmount>,
    /// Assets offered in exchange
    pub offered_assets: Vec<OfferAmount>,
    /// Transaction fee
    pub fee: Amount,
    /// Optional receive address
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(nullable = true))]
    pub receive_address: Option<String>,
    /// Optional expiration timestamp
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(nullable = true))]
    pub expires_at_second: Option<u64>,
    /// Optional specific coin IDs to use for the offer instead of auto-selecting
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(nullable = true))]
    pub coin_ids: Option<Vec<String>>,
}

/// Response with the unsigned offer coin spends
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MakeOfferUnsignedResponse {
    /// Unsigned coin spends making up the offer
    pub coin_spends: Vec<CoinSpendJson>,
}

/// Assemble and encode an offer from externally signed coin spends.
///
/// Counterpart to [`MakeOfferUnsigned`]: takes the coin spends and the BLS
/// signatures produced by an external signer (e.g. a Tangem card), aggregates
/// them into a spend bundle, and encodes it as a bech32 offer string.
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Aggregate externally produced BLS signatures into the offer's coin spends and encode the finished offer. Pair with `make_offer_unsigned` and `required_signatures`."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct EncodeOffer {
    /// Coin spends that were signed
    pub coin_spends: Vec<CoinSpendJson>,
    /// Hex-encoded BLS signatures to aggregate (order does not matter)
    pub signatures: Vec<String>,
    /// Whether to automatically import the offer into the local database
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(default = false))]
    pub auto_import: bool,
}

/// Response with the encoded offer
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct EncodeOfferResponse {
    /// Offer string (bech32 encoded)
    pub offer: String,
    /// Offer ID
    pub offer_id: String,
}

/// Accept an offer
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Accept and complete an offer created by another party."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TakeOffer {
    /// Offer string to accept
    pub offer: String,
    /// Transaction fee
    pub fee: Amount,
    /// Whether to automatically submit the transaction
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(default = false))]
    pub auto_submit: bool,
}

/// Response with accepted offer details
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TakeOfferResponse {
    /// Transaction summary
    pub summary: TransactionSummary,
    /// Spend bundle
    pub spend_bundle: SpendBundleJson,
    /// Transaction ID
    pub transaction_id: String,
}

/// Build the coin spends to take an offer without signing them.
///
/// Same as [`TakeOffer`] but returns the unsigned coin spends so an external
/// signer (e.g. a Tangem card) can produce the BLS signatures. Pair with
/// `required_signatures` for the messages to sign, then `submit_with_signatures`
/// to aggregate and broadcast the taker spend bundle.
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Build the coin spends to take an offer without signing them, for external/hardware signers such as Tangem cards. Use `required_signatures` for the messages to sign, then `submit_with_signatures` to broadcast."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TakeOfferUnsigned {
    /// Offer string to accept
    pub offer: String,
    /// Transaction fee
    pub fee: Amount,
}

/// Response with the unsigned taker coin spends
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TakeOfferUnsignedResponse {
    /// Unsigned coin spends for taking the offer
    pub coin_spends: Vec<CoinSpendJson>,
}

/// Combine multiple offers
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Combine multiple offers into a single compound offer."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CombineOffers {
    /// Offer strings to combine
    pub offers: Vec<String>,
}

/// Response with combined offer
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CombineOffersResponse {
    /// Combined offer string
    pub offer: String,
}

/// View an offer without accepting
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "View the details of an offer without accepting it."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ViewOffer {
    /// Offer string to view
    pub offer: String,
}

/// Response with offer details
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ViewOfferResponse {
    /// Offer summary
    pub offer: OfferSummary,
    /// Offer status
    pub status: OfferRecordStatus,
}

/// Import an offer
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Import an offer file from an external source."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ImportOffer {
    /// Offer string to import
    pub offer: String,
}

/// Response with imported offer ID
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ImportOfferResponse {
    /// ID of the imported offer
    pub offer_id: String,
}

/// List all offers
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "List all offers created by or available to this wallet."
    )
)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct GetOffers {}

/// Response with list of offers
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct GetOffersResponse {
    /// List of offers
    pub offers: Vec<OfferRecord>,
}

/// Get offers for a specific asset
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Get all offers that involve a specific asset."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct GetOffersForAsset {
    /// Asset ID to filter by
    pub asset_id: String,
}

/// Response with offers for asset
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct GetOffersForAssetResponse {
    /// List of offers involving the asset
    pub offers: Vec<OfferRecord>,
}

/// Get a specific offer
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Get detailed information about a specific offer by ID."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct GetOffer {
    /// Offer ID
    pub offer_id: String,
}

/// Response with offer details
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct GetOfferResponse {
    /// Offer details
    pub offer: OfferRecord,
}

/// Delete an offer
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Delete an offer from the wallet (doesn't cancel on-chain)."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeleteOffer {
    /// Offer ID to delete
    pub offer_id: String,
}

/// Response for offer deletion
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Offers"))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeleteOfferResponse {}

/// Cancel an offer on-chain
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Cancel an offer by spending the offered coins on-chain.",
        response_type = "TransactionResponse"
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CancelOffer {
    /// Offer ID to cancel
    pub offer_id: String,
    /// Transaction fee
    pub fee: Amount,
    /// Whether to automatically submit the transaction
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(default = false))]
    pub auto_submit: bool,
}

pub type CancelOfferResponse = TransactionResponse;

/// Cancel multiple offers
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Offers",
        description = "Cancel multiple offers in a single transaction.",
        response_type = "TransactionResponse"
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CancelOffers {
    /// Offer IDs to cancel
    pub offer_ids: Vec<String>,
    /// Transaction fee
    pub fee: Amount,
    /// Whether to automatically submit the transaction
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(default = false))]
    pub auto_submit: bool,
}

pub type CancelOffersResponse = TransactionResponse;

fn yes() -> bool {
    true
}
