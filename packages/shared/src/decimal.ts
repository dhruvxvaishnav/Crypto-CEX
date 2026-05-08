import DecimalJs from "decimal.js";

export type DecimalInput = Decimal | DecimalJs.Value;

export class Decimal {
  readonly #value: DecimalJs;

  private constructor(value: DecimalJs) {
    this.#value = value;
  }

  static from(input: DecimalInput): Decimal {
    if (input instanceof Decimal) {
      return input;
    }

    return new Decimal(new DecimalJs(input));
  }

  add(input: DecimalInput): Decimal {
    return new Decimal(this.#value.add(Decimal.from(input).#value));
  }

  mul(input: DecimalInput): Decimal {
    return new Decimal(this.#value.mul(Decimal.from(input).#value));
  }

  toFixedString(): string {
    return this.#value.toFixed();
  }
}

export function D(input: DecimalInput): Decimal {
  return Decimal.from(input);
}
