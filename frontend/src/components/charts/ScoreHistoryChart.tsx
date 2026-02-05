import { onMount, onCleanup, createEffect, on, createSignal } from 'solid-js'
import { createChart, ColorType, LineSeries, HistogramSeries } from 'lightweight-charts'
import type { IChartApi, ISeriesApi, LineData, HistogramData } from 'lightweight-charts'
import type { ScoreHistorySummary } from '../../api/client'

export interface ScoreHistoryChartProps {
  data: ScoreHistorySummary[]
  height?: number
  showRank?: boolean
  colors?: {
    background?: string
    text?: string
    grid?: string
    scoreColor?: string
    rankColor?: string
    positiveChange?: string
    negativeChange?: string
  }
}

// RouteState에 따른 색상 매핑
const ROUTE_STATE_COLORS: Record<string, string> = {
  'Attack': 'rgba(34, 197, 94, 0.15)',    // 녹색 (적극적 진입)
  'Armed': 'rgba(59, 130, 246, 0.15)',    // 파랑 (준비 상태)
  'Weak': 'rgba(234, 179, 8, 0.15)',      // 노랑 (약세)
  'Exit': 'rgba(239, 68, 68, 0.15)',      // 빨강 (청산)
  'Wait': 'rgba(107, 114, 128, 0.1)',     // 회색 (관망)
}

// 데이터를 시간순으로 정렬
function sortByTime(data: ScoreHistorySummary[]): ScoreHistorySummary[] {
  return [...data].sort((a, b) => a.score_date.localeCompare(b.score_date))
}

export function ScoreHistoryChart(props: ScoreHistoryChartProps) {
  let containerRef: HTMLDivElement | undefined
  let chart: IChartApi | undefined
  let scoreSeries: ISeriesApi<'Line'> | undefined
  let changeHistogram: ISeriesApi<'Histogram'> | undefined

  const defaultColors = {
    background: 'transparent',
    text: '#d1d5db',
    grid: '#374151',
    scoreColor: '#8b5cf6',      // 보라색 (Global Score)
    rankColor: '#f59e0b',       // 주황색 (Rank)
    positiveChange: '#22c55e',  // 녹색 (점수 상승)
    negativeChange: '#ef4444',  // 빨강 (점수 하락)
  }

  const getColors = () => ({
    ...defaultColors,
    ...props.colors,
  })

  onMount(() => {
    if (!containerRef) return

    const colors = getColors()

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
      height: props.height || 200,
      crosshair: {
        mode: 1,
      },
      timeScale: {
        borderColor: colors.grid,
        timeVisible: true,
      },
      rightPriceScale: {
        borderColor: colors.grid,
        scaleMargins: {
          top: 0.1,
          bottom: 0.3, // 히스토그램 공간 확보
        },
      },
    })

    // Global Score 라인 차트
    scoreSeries = chart.addSeries(LineSeries, {
      color: colors.scoreColor,
      lineWidth: 2,
      priceFormat: {
        type: 'custom',
        formatter: (price: number) => price.toFixed(1),
      },
    })

    // 점수 변화 히스토그램 (하단)
    changeHistogram = chart.addSeries(HistogramSeries, {
      priceFormat: {
        type: 'custom',
        formatter: (price: number) => (price >= 0 ? '+' : '') + price.toFixed(1),
      },
      priceScaleId: 'change',
    })

    // 히스토그램용 별도 스케일
    chart.priceScale('change').applyOptions({
      scaleMargins: {
        top: 0.8,
        bottom: 0,
      },
    })

    // 초기 데이터 설정
    if (props.data && props.data.length > 0) {
      updateChartData(props.data)
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

  // 차트 데이터 업데이트 함수
  const updateChartData = (data: ScoreHistorySummary[]) => {
    if (!scoreSeries || !changeHistogram) return

    const sortedData = sortByTime(data)
    const colors = getColors()

    // Global Score 라인 데이터
    const scoreData: LineData[] = sortedData
      .filter(d => d.global_score !== null)
      .map(d => ({
        time: d.score_date,
        value: d.global_score!,
      }))

    // 점수 변화 히스토그램 데이터
    const changeData: HistogramData[] = sortedData
      .filter(d => d.score_change !== null)
      .map(d => ({
        time: d.score_date,
        value: d.score_change!,
        color: d.score_change! >= 0 ? colors.positiveChange : colors.negativeChange,
      }))

    scoreSeries.setData(scoreData)
    changeHistogram.setData(changeData)
    chart?.timeScale().fitContent()
  }

  // 데이터 변경 감지
  createEffect(
    on(
      () => props.data,
      (data) => {
        if (data && data.length > 0) {
          updateChartData(data)
        }
      }
    )
  )

  // 마지막 점수 및 변화량 계산
  const lastData = () => {
    if (!props.data || props.data.length === 0) return null
    const sorted = sortByTime(props.data)
    return sorted[sorted.length - 1]
  }

  return (
    <div class="relative">
      {/* 차트 컨테이너 */}
      <div
        ref={containerRef}
        class="w-full rounded-lg overflow-hidden"
        style={{ height: `${props.height || 200}px` }}
      />

      {/* 범례 및 현재 값 표시 */}
      <div class="absolute top-2 left-2 flex gap-4 text-xs">
        <div class="flex items-center gap-1">
          <div class="w-3 h-0.5" style={{ background: getColors().scoreColor }} />
          <span class="text-gray-400">Global Score</span>
          {lastData()?.global_score !== null && (
            <span class="font-semibold text-white ml-1">
              {lastData()?.global_score?.toFixed(1)}
            </span>
          )}
        </div>

        {lastData()?.score_change !== null && (
          <div class="flex items-center gap-1">
            <span
              class="font-semibold"
              style={{
                color: (lastData()?.score_change ?? 0) >= 0
                  ? getColors().positiveChange
                  : getColors().negativeChange
              }}
            >
              {(lastData()?.score_change ?? 0) >= 0 ? '+' : ''}
              {lastData()?.score_change?.toFixed(1)}
            </span>
          </div>
        )}

        {lastData()?.route_state && (
          <div class="flex items-center gap-1">
            <div
              class="px-1.5 py-0.5 rounded text-xs font-medium"
              style={{
                background: ROUTE_STATE_COLORS[lastData()?.route_state ?? 'Wait'] ?? ROUTE_STATE_COLORS.Wait,
                color: '#fff'
              }}
            >
              {lastData()?.route_state}
            </div>
          </div>
        )}
      </div>

      {/* Rank 표시 (선택적) */}
      {props.showRank && lastData()?.rank !== null && (
        <div class="absolute top-2 right-2 text-xs">
          <span class="text-gray-400">Rank: </span>
          <span class="font-semibold text-amber-500">#{lastData()?.rank}</span>
          {lastData()?.rank_change !== null && lastData()?.rank_change !== 0 && (
            <span
              class="ml-1"
              style={{
                color: (lastData()?.rank_change ?? 0) < 0  // 랭크는 낮을수록 좋음
                  ? getColors().positiveChange
                  : getColors().negativeChange
              }}
            >
              ({(lastData()?.rank_change ?? 0) < 0 ? '' : '+'}{lastData()?.rank_change})
            </span>
          )}
        </div>
      )}
    </div>
  )
}

export default ScoreHistoryChart
