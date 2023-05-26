module aptos_framework::transaction_context {
    /// Return new globally unique identifier.
    public native fun create_guid(): address;

    /// Return the script hash of the current entry function.
    public native fun get_script_hash(): vector<u8>;
}
