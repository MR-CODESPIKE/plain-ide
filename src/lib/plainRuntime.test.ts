import { describe, expect, it } from 'vitest';
import { checkPlain, runPlain } from './plainRuntime';

const sample = `LET evenTotal = 0
REPEAT n FROM 1 TO 10
  IF n % 2 = 0 THEN
    SET evenTotal = evenTotal + n
  ELSE
    SAY "odd: " + n
  END IF
END REPEAT
SAY "even total = " + evenTotal`;

describe('Plain browser preview runner', () => {
  it('runs variables, counted loops, conditionals, modulo, and output', () => {
    const result = runPlain(sample);
    expect(result.ok).toBe(true);
    expect(result.output).toEqual(['odd: 1', 'odd: 3', 'odd: 5', 'odd: 7', 'odd: 9', 'even total = 30']);
    expect(result.steps).toBeGreaterThan(10);
  });

  it('checks valid syntax and reports line-numbered parse errors', () => {
    expect(checkPlain('LET x = 4\nSAY x').ok).toBe(true);
    expect(checkPlain('IF TRUE THEN\nSAY "ok"').message).toContain('line 1');
    expect(runPlain('SET missing = 2').error).toContain('unknown variable');
  });

  it('keeps comments and quoted hash characters distinct', () => {
    const result = runPlain('LET label = "A # mark" # trailing comment\nSAY label');
    expect(result.output).toEqual(['A # mark']);
  });

  it('reports arithmetic faults without crashing the caller', () => {
    expect(runPlain('LET x = 4 / 0').error).toContain('Division by zero');
    expect(runPlain('LET x = (3 + 2').error).toContain('Expected )');
  });

  it('rejects an excessive counted loop before iteration', () => {
    const result = runPlain('REPEAT i FROM 1 TO 50001\nSAY i\nEND REPEAT');
    expect(result.ok).toBe(false);
    expect(result.error).toContain('Loop range exceeds');
  });

  it('rejects non-finite arithmetic before it can stall a counted loop', () => {
    const huge = '9'.repeat(400);
    const result = runPlain(`LET huge = ${huge} * ${huge}\nREPEAT i FROM huge TO huge\nSAY i\nEND REPEAT`);
    expect(result.ok).toBe(false);
    expect(result.error).toContain('finite number');
  });

  it('rejects counted-loop bounds that cannot be incremented safely', () => {
    const result = runPlain('REPEAT i FROM 9007199254740992 TO 9007199254740992\nSAY i\nEND REPEAT');
    expect(result.ok).toBe(false);
    expect(result.error).toContain('safe whole numbers');
  });
});
