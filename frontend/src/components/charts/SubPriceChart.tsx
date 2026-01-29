/**
 * SubPriceChart - 별도 패널 지표 차트 컴포넌트.
 *
 * RSI, MACD, Stochastic 등 가격과 다른 스케일을 가진 지표를 별도 패널로 표시합니다.
 * Lightweight Charts를 사용하여 메인 가격 차트와 시간 축을 동기화합니다.
 */
import { onMount, onCleanup, createEffect, on, For, Show } from 'solid-js'
import { createChart, ColorType, LineSeries, HistogramSeries } from 'lightweight-charts'
import type { IChartApi, ISeriesApi, LineData, HistogramData, Time } from 'lightweight-charts'
import type { LineDataPoint } from './PriceChart'

/** 지표 시리즈 데이터 */
export interface IndicatorSeriesData {
  name: string
  data: LineDataPoint[]
  color: string
  seriesType: 'line' | 'bar' | 'area'
  lineWidth?: number
}

/** 별도 패널 지표 데이터 */
export interface SeparateIndicatorData {
  id: string
  type: string
  name: string
  series: IndicatorSeriesData[]
  scaleRange?: { min: number; max: number; levels?: number[] }
}

interface SubPriceChartProps {
  /** 지표 데이터 */
  indicator: SeparateIndicatorData
  /** 차트 높이 */
  height?: number
  /** 시간 범위 동기화를 위한 참조 (선택적) */
  timeRangeRef?: { from: Time; to: Time }
  /** 시간 범위 변경 콜백 */
  onTimeRangeChange?: (from: Time, to: Time) => void
  /** 커스텀 색상 */
  colors?: {
    background?: string
    text?: string
    grid?: string
  }
}

export function SubPriceChart(props: SubPriceChartProps) {
  let containerRef: HTMLDivElement | undefined
  let chart: IChartApi | undefined
  const seriesMap = new Map<string, ISeriesApi<'Line'> | ISeriesApi<'Histogram'>>()

  const defaultColors = {
    background: 'transparent',
    text: '#d1d5db',
    grid: '#374151',
  }

  const getColors = () => ({
    ...defaultColors,
    ...props.colors,
  })

  onMount(() => {
    if (!containerRef) return

    const colors = getColors()
    const height = props.height || 120

    chart = createChart(containerRef, {
      layout: {
        background: { type: ColorType.Solid, color: colors.background },
        textColor: colors.text,
      },
      grid: {
        vertLines: { color: colors.grid },
        horzLines: { color: colors.grid },
      },
      width: containerRef.clientWidth,
      height,
      crosshair: {
        mode: 1,
      },
      timeScale: {
        borderColor: colors.grid,
        timeVisible: true,
        secondsVisible: false,
        visible: true,
      },
      rightPriceScale: {
        borderColor: colors.grid,
        scaleMargins: {
          top: 0.1,
          bottom: 0.1,
        },
      },
    })

    // 초기 시리즈 생성
    createIndicatorSeries()

    // 시간 범위 변경 리스너
    if (props.onTimeRangeChange) {
      chart.timeScale().subscribeVisibleTimeRangeChange((range) => {
        if (range && props.onTimeRangeChange) {
          props.onTimeRangeChange(range.from as Time, range.to as Time)
        }
      })
    }

    // 리사이즈 핸들러
    const handleResize = () => {
      if (chart && containerRef) {
        chart.applyOptions({ width: containerRef.clientWidth })
      }
    }

    window.addEventListener('resize', handleResize)

    onCleanup(() => {
      window.removeEventListener('resize', handleResize)
      if (chart) {
        chart.remove()
      }
    })
  })

  // 지표 시리즈 생성
  function createIndicatorSeries() {
    if (!chart) return

    const indicator = props.indicator
    if (!indicator.series) return

    for (const series of indicator.series) {
      const lineWidth = (series.lineWidth && series.lineWidth >= 1 && series.lineWidth <= 4
        ? series.lineWidth
        : 2) as 1 | 2 | 3 | 4

      let chartSeries: ISeriesApi<'Line'> | ISeriesApi<'Histogram'>

      if (series.seriesType === 'bar') {
        // 히스토그램 (MACD 히스토그램 등)
        chartSeries = chart.addSeries(HistogramSeries, {
          color: series.color,
          priceFormat: {
            type: 'price',
            precision: 4,
            minMove: 0.0001,
          },
        })
      } else {
        // 라인 (RSI, MACD 라인, Stochastic 등)
        chartSeries = chart.addSeries(LineSeries, {
          color: series.color,
          lineWidth,
          lastValueVisible: true,
          priceLineVisible: false,
        })
      }

      // 데이터 설정
      if (series.data && series.data.length > 0) {
        const sortedData = [...series.data].sort((a, b) => {
          const timeA = typeof a.time === 'string' ? a.time : a.time.toString()
          const timeB = typeof b.time === 'string' ? b.time : b.time.toString()
          return timeA.localeCompare(timeB)
        })

        if (series.seriesType === 'bar') {
          // 히스토그램 데이터 - 양수/음수에 따라 색상 변경
          const histogramData = sortedData.map(d => ({
            time: d.time as Time,
            value: d.value,
            color: d.value >= 0 ? '#22c55e' : '#ef4444',
          }))
          chartSeries.setData(histogramData as HistogramData[])
        } else {
          chartSeries.setData(sortedData as LineData[])
        }
      }

      seriesMap.set(series.name, chartSeries)
    }

    // 기준선 추가 (RSI 30/70, Stochastic 20/80 등)
    if (indicator.scaleRange?.levels) {
      for (const level of indicator.scaleRange.levels) {
        const levelSeries = chart.addSeries(LineSeries, {
          color: '#6b7280',
          lineWidth: 1,
          lineStyle: 2, // Dashed
          lastValueVisible: false,
          priceLineVisible: false,
        })

        // 모든 데이터 포인트에 대해 수평선 생성
        const firstSeries = indicator.series[0]
        if (firstSeries?.data?.length > 0) {
          const levelData = firstSeries.data.map(d => ({
            time: d.time as Time,
            value: level,
          }))
          levelSeries.setData(levelData as LineData[])
        }
      }
    }

    // 차트 스케일 조정
    requestAnimationFrame(() => {
      chart?.timeScale().fitContent()
    })
  }

  // 데이터 변경 시 업데이트
  createEffect(
    on(
      () => props.indicator,
      (indicator) => {
        if (!chart || !indicator) return

        // 기존 시리즈 제거
        for (const series of seriesMap.values()) {
          try {
            chart.removeSeries(series)
          } catch {
            // 이미 제거된 시리즈 무시
          }
        }
        seriesMap.clear()

        // 새 시리즈 생성
        createIndicatorSeries()
      }
    )
  )

  // 시간 범위 동기화
  createEffect(
    on(
      () => props.timeRangeRef,
      (timeRange) => {
        if (!chart || !timeRange) return
        try {
          chart.timeScale().setVisibleRange({
            from: timeRange.from,
            to: timeRange.to,
          })
        } catch {
          // 범위 설정 실패 무시
        }
      }
    )
  )

  return (
    <div class="mt-2">
      {/* 지표 이름 레이블 */}
      <div class="flex items-center gap-2 mb-1 px-2">
        <span class="text-xs font-medium text-[var(--color-text-muted)]">
          {props.indicator.name}
        </span>
        <div class="flex gap-2">
          <For each={props.indicator.series}>
            {(series) => (
              <span
                class="inline-flex items-center gap-1 text-xs"
                style={{ color: series.color }}
              >
                <span
                  class="w-2 h-0.5 rounded"
                  style={{ 'background-color': series.color }}
                />
                {series.name}
              </span>
            )}
          </For>
        </div>
      </div>

      {/* 차트 컨테이너 */}
      <div
        ref={containerRef}
        class="w-full rounded-lg overflow-hidden border border-[var(--color-surface-light)]"
        style={{ height: `${props.height || 120}px` }}
      />

      {/* 기준선 레이블 (있는 경우) */}
      <Show when={props.indicator.scaleRange?.levels}>
        <div class="flex gap-2 mt-1 px-2">
          <For each={props.indicator.scaleRange?.levels}>
            {(level) => (
              <span class="text-xs text-[var(--color-text-muted)]">
                {level}
              </span>
            )}
          </For>
        </div>
      </Show>
    </div>
  )
}
