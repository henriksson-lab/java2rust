package accessorboundaries;
public class Cell {
    private int value;
    Cell(int value) { this.value=value; }
    int read() { return value; }
    void write(int value) { this.value=value; }
}
