package accessorboundaries;
public class Holder {
    private final Cell payload;
    Holder(Cell payload) { this.payload=payload; }
    Cell payload() { return payload; }
}
