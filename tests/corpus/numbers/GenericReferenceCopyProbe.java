class GenericReferenceCell<T> {
    int updates;
    void bump() { updates++; }
}

class GenericReferenceCopyProbe {
    static int run() {
        GenericReferenceCell<String> value = new GenericReferenceCell<>();
        GenericReferenceCell<String> alias = value;
        alias.bump();
        Object erased = value;
        Object copied = erased;
        if (copied != erased || copied != value) return 1;
        alias.bump();
        if (value.updates != 2) return 2;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
