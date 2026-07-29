class StaleConstructorOptionalEnumFieldShapeBase {
    String key;

    StaleConstructorOptionalEnumFieldShapeBase(String key, String value) {
        this.key = key;
    }
}

class StaleConstructorOptionalEnumFieldShape extends StaleConstructorOptionalEnumFieldShapeBase {
    enum CountType { INTEGER, UNBOUNDED }
    enum LineType { FORMAT, INFO }

    CountType countType = null;
    LineType lineType = null;
    int count;

    StaleConstructorOptionalEnumFieldShape(int count, LineType lineType) {
        this(count, lineType, null);
    }

    StaleConstructorOptionalEnumFieldShape(int count, LineType lineType, String source) {
        super(lineType.toString(), "");
        this.count = count;
        this.countType = CountType.INTEGER;
        this.lineType = lineType;
    }

    StaleConstructorOptionalEnumFieldShape(CountType countType, LineType lineType, String source) {
        super(lineType.toString(), "");
        this.countType = countType;
        this.lineType = lineType;
    }

    void setNumberToUnbounded() {
        this.countType = CountType.UNBOUNDED;
        this.count = -1;
    }
}
