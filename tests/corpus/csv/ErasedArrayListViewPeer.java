
class ErasedArrayListViewPeer {
    static Iterable<Object> forward(Object values) { return ErasedArrayListViewContracts.view(values); }
    static int run() {
        String[] values = new String[] { "peer" };
        Iterable<Object> iterable = forward(values);
        values[0] = "peer-late";
        return iterable.iterator().next() == values[0] ? 0 : 1;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
