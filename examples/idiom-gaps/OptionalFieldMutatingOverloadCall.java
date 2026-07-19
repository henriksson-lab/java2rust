class OptionalFieldMutatingOverloadCall {
    Holder holder;

    void apply(String name, int value) {
        holder.add(name, value);
    }
}

class Holder {
    int count;

    void add(String name, int value) {
        count = value;
    }

    void add(String name, int value, int extra) {
        count = value + extra;
    }
}
