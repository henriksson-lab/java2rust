class VirtualCallbackContractsOracle {
    public static void main(String[] args) {
        if (VirtualCallbackContracts.forwarded() != 10
                || VirtualCallbackContracts.direct() != 7
                || VirtualCallbackContracts.dispatched() != 8
                || VirtualCallbackContracts.retained() != 6
                || VirtualCallbackContracts.caught() != 9) {
            throw new AssertionError("virtual callback value contract");
        }
        System.out.println("virtual-callback-contracts: ok");
    }
}
