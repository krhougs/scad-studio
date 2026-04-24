import { NumberField } from "@base-ui/react/number-field";
import {
  KnobHeadless,
  useKnobKeyboardControls,
} from "react-knob-headless";
import { useEffect, useState } from "react";
import type React from "react";

type NumericControlProps = {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  inputTestId: string;
  knobTestId: string;
  numberFieldTestId: string;
  controlTestId?: string;
  fractionDigits?: number;
  onChange: (value: number) => void;
};

export function NumericControl({
  label,
  value,
  min,
  max,
  step,
  inputTestId,
  knobTestId,
  numberFieldTestId,
  controlTestId,
  fractionDigits,
  onChange,
}: NumericControlProps) {
  const range = normalizedRange(min, max);
  const raw = Number.isFinite(value) ? value : 0;
  const [fieldValue, setFieldValue] = useState<number | null>(raw);
  const commitStepValue = (next: number) => {
    onChange(roundToStep(next, step));
  };
  useEffect(() => {
    setFieldValue(raw);
  }, [raw]);
  const knobValue = clamp(raw, range.min, range.max);
  const progress = ratio(knobValue, range.min, range.max);
  const keyboard = useKnobKeyboardControls({
    valueRaw: knobValue,
    valueMin: range.min,
    valueMax: range.max,
    step,
    stepLarger: step * 10,
    onValueRawChange: commitStepValue,
  });
  const format =
    typeof fractionDigits === "number"
      ? { minimumFractionDigits: fractionDigits, maximumFractionDigits: fractionDigits }
      : undefined;
  const renderInput = (
    props: React.InputHTMLAttributes<HTMLInputElement> &
      React.RefAttributes<HTMLInputElement>,
  ) => (
    <input
      {...props}
      type={typeof fractionDigits === "number" ? "text" : "number"}
      role={typeof fractionDigits === "number" ? "spinbutton" : props.role}
      onFocus={(event) => {
        props.onFocus?.(event);
        if (typeof fractionDigits === "number") event.currentTarget.select();
      }}
    />
  );

  return (
    <div
      className="numeric-control"
      data-testid={controlTestId}
      style={{ "--knob-progress": String(progress) } as React.CSSProperties}
    >
      <KnobHeadless
        aria-label={`${label} knob`}
        className="numeric-control__knob"
        data-testid={knobTestId}
        valueRaw={knobValue}
        valueMin={range.min}
        valueMax={range.max}
        dragSensitivity={0.006}
        valueRawRoundFn={(next) => roundToStep(next, step)}
        valueRawDisplayFn={(next) => formatNumber(next, fractionDigits)}
        onValueRawChange={commitStepValue}
        includeIntoTabOrder
        {...keyboard}
      >
        <span className="numeric-control__knob-track" aria-hidden="true" />
        <span className="numeric-control__knob-pointer" aria-hidden="true" />
      </KnobHeadless>
      <NumberField.Root
        className="numeric-control__number-field"
        data-testid={numberFieldTestId}
        value={fieldValue}
        min={range.min}
        max={range.max}
        step={step}
        smallStep={step}
        largeStep={step * 10}
        allowOutOfRange
        format={format}
        onValueChange={(next) => {
          setFieldValue(next);
          if (typeof next === "number" && Number.isFinite(next)) onChange(next);
        }}
        onValueCommitted={(next) => {
          if (next === null) setFieldValue(raw);
        }}
      >
        <NumberField.Group className="numeric-control__number-group">
          <NumberField.Decrement className="numeric-control__stepper">
            -
          </NumberField.Decrement>
          <NumberField.Input
            className="numeric-control__input"
            data-testid={inputTestId}
            aria-label={label}
            render={renderInput}
            aria-valuemin={range.min}
            aria-valuemax={range.max}
            aria-valuenow={fieldValue ?? undefined}
          />
          <NumberField.Increment className="numeric-control__stepper">
            +
          </NumberField.Increment>
        </NumberField.Group>
      </NumberField.Root>
    </div>
  );
}

export function normalizedRange(min: number, max: number): { min: number; max: number } {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return { min: -1, max: 1 };
  if (min < max) return { min, max };
  const center = min;
  return { min: center - 1, max: center + 1 };
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function roundToStep(value: number, step: number): number {
  if (!Number.isFinite(value) || !Number.isFinite(step) || step <= 0) return value;
  const rounded = Math.round(value / step) * step;
  return Number(rounded.toFixed(decimalPlaces(step)));
}

export function formatNumber(value: number, fractionDigits?: number): string {
  if (!Number.isFinite(value)) return "0";
  if (typeof fractionDigits === "number") return value.toFixed(fractionDigits);
  return String(value);
}

function ratio(value: number, min: number, max: number): number {
  const span = max - min;
  if (span <= 0) return 0.5;
  return clamp((value - min) / span, 0, 1);
}

function decimalPlaces(value: number): number {
  const text = String(value);
  if (!text.includes(".")) return 0;
  return Math.min(text.split(".")[1]?.length ?? 0, 8);
}
