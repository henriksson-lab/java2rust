interface NullableArgumentData { int number(); }
class NullableArgumentPayload implements NullableArgumentData {
    final int value;
    NullableArgumentPayload(int value) { this.value = value; }
    public int number() { return value; }
}
class NullableArgumentOwner {
    NullableArgumentPayload payload;
    NullableArgumentData data;
    NullableArgumentOwner(NullableArgumentPayload payload) {
        this.payload = payload;
    }
    NullableArgumentPayload read() { return payload; }
    void write(NullableArgumentData value) { data = value; }
    void copy(NullableArgumentOwner next) { next.write(next.read()); }
}
class NullableReceiverArgumentProbe {
    static int run() {
        NullableArgumentOwner owner = new NullableArgumentOwner(new NullableArgumentPayload(7));
        NullableArgumentOwner alias = owner;
        owner.copy(alias);
        if (owner.data.number() != 7 || alias != owner) return 1;
        NullableArgumentOwner missing = new NullableArgumentOwner(null);
        NullableArgumentOwner missingAlias = missing;
        missing.copy(missing);
        if (missing.data != null || missingAlias != missing) return 2;
        owner.copy(missing);
        if (owner.data.number() != 7 || missing.data != null) return 3;
        owner.copy(owner);
        if (owner.data.number() != 7) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
