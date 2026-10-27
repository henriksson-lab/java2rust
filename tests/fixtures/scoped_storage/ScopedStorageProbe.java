class ScopedStorageProbe {
    static int run() {
        Cell original = new Cell();
        Cell alias = original;
        alias.value = 7;
        if (original.value != 7) return 1;
        Unrelated<Cell> holder = new Unrelated<Cell>(original);
        holder.get().value = 9;
        if (original.value != 9) return 2;
        return ScopedValueReferences.run() == 13 ? 0 : 3;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
