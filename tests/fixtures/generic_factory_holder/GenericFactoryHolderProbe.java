interface TypedProducer<P extends TypedProduct<?>> { P make(); }
class TypedValue<T> implements TypedProduct<T> {
    T value;
    TypedValue(T value) { this.value = value; }
    public T get() { return value; }
    public void set(T value) { this.value = value; }
}
class TypedFactory<T> implements TypedProducer<TypedValue<T>> {
    T seed;
    TypedFactory(T seed) { this.seed = seed; }
    public TypedValue<T> make() { return new TypedValue<T>(seed); }
}
class TypedHolder<T> {
    TypedProduct<T> value;
    TypedHolder(TypedProducer<? extends TypedProduct<T>> source) { value = source.make(); }
    TypedProduct<T> get() { return value; }
    TypedProduct<T> replace(TypedProducer<? extends TypedProduct<T>> source) {
        TypedProduct<T> replacement = source.make();
        java.util.List<TypedProduct<T>> copies = new java.util.ArrayList<TypedProduct<T>>();
        copies.add(replacement);
        copies.add(source.make());
        value = copies.get(0);
        return copies.get(1);
    }
}
class GenericFactoryHolderProbe {
    enum Token { FIRST, SECOND }
    static int run() {
        TypedHolder<Token> holder = new TypedHolder<Token>(new TypedFactory<Token>(Token.FIRST));
        TypedProduct<Token> product = holder.get();
        if (product.get() != Token.FIRST) return 1;
        product.set(Token.SECOND);
        if (holder.get().get() != Token.SECOND) return 2;
        TypedProduct<Token> other = holder.replace(new TypedFactory<Token>(Token.FIRST));
        if (holder.get() == product || holder.get().get() != Token.FIRST) return 3;
        if (other == holder.get() || other.get() != Token.FIRST) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
