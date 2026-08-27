public class InheritanceUpcast {
    public static String run() {
        Account a = new Savings(100, 5);
        return a.label() + "," + a.balance() + "," + a.describe();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class Account {
    int amount;

    Account(int amount) {
        this.amount = amount;
    }

    String label() {
        return "account";
    }

    int balance() {
        return amount;
    }

    String describe() {
        return label() + ":" + balance();
    }
}

class Savings extends Account {
    int rate;

    Savings(int amount, int rate) {
        super(amount);
        this.rate = rate;
    }

    String label() {
        return "savings";
    }

    int balance() {
        return amount + rate;
    }
}
