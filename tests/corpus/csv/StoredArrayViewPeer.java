class StoredArrayViewPeer {
    static Object[] relay(Object[] values) { return StoredArrayViewContracts.forward(values); }
    static int run() {
        String[] values = new String[] { "peer", null };
        Object[] alias = relay(values);
        alias[0] = "peer-late";
        return alias == values && values[0].equals("peer-late") ? 0 : 1;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
