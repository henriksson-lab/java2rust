/**
 * Assigning a subclass instance to a supertype-typed variable and then calling
 * through it. Java dispatches to the subclass override and keeps the subclass
 * state; the translation lowers the upcast as `Savings::new(..).into()` and
 * generates `impl From<Savings> for Account` that builds a *fresh* `Account`
 * from the base fields only, so both the override and the subclass field are
 * lost.
 *
 * Java prints
 *
 *     savings,105,savings:105
 *
 * The translation compiles cleanly and prints
 *
 *     account,100,account:100
 *
 * The translator does report `unsupported_inheritance` on stderr, but it still
 * emits a program that runs and answers wrongly, so nothing downstream can tell
 * the difference. See the audit for the nested-class variant, which instead
 * fails to compile because no `From` impl is generated at all.
 */
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
