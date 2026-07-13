class SerializableValueObject implements Serializable {
    private static final long serialVersionUID = 7L;
    private final String name;
    private final int count;

    SerializableValueObject(String name, int count) {
        this.name = name;
        this.count = count;
    }

    Object roundTrip(ObjectOutputStream out, ObjectInputStream in) throws IOException, ClassNotFoundException {
        out.writeObject(this);
        return in.readObject();
    }
}
